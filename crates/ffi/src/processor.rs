use std::ffi::CString;
use std::path::Path;
use std::ptr::NonNull;

use crate::bindings as raw;
use snafu::ResultExt;

use crate::error::{Error, OpenSnafu};

/// The color space LibRaw converts to (`params.output_color`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputColor {
    /// The camera's own color space, unconverted.
    Raw,
    Srgb,
}

/// Bits per sample in processed output (`params.output_bps`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputBps {
    Eight,
    Sixteen,
}

/// The encoding of a `MemImage`'s bytes (`LibRaw_image_formats`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Bitmap,
    JpegXl,
    H265,
    Other(u32),
}

impl ImageFormat {
    fn from_raw(format: raw::LibRaw_image_formats) -> Self {
        match format {
            raw::LibRaw_image_formats_LIBRAW_IMAGE_JPEG => Self::Jpeg,
            raw::LibRaw_image_formats_LIBRAW_IMAGE_BITMAP => Self::Bitmap,
            raw::LibRaw_image_formats_LIBRAW_IMAGE_JPEGXL => Self::JpegXl,
            raw::LibRaw_image_formats_LIBRAW_IMAGE_H265 => Self::H265,
            other => Self::Other(other),
        }
    }
}

/// A processed image or thumbnail copied out of LibRaw's memory, as LibRaw
/// describes it. For a `Bitmap` from `make_mem_image`, `data` holds
/// `width * height * colors` samples of `bits` each, row-major. A bitmap
/// thumbnail holds whatever the camera stored, so check `data` against the
/// fields before trusting them. For the compressed formats `data` is the
/// encoded stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemImage {
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
    pub bits: u16,
    pub colors: u16,
    pub data: Vec<u8>,
}

/// One LibRaw instance (`libraw_data_t`). Each method is one C API call that
/// checks its return code. Calls go in LibRaw's order: `open_file`, then
/// `unpack` or `unpack_thumb`, then the setters and `dcraw_process`, then
/// `make_mem_image` or `make_mem_thumb`. An out-of-order call comes back as
/// an `Error` with `ErrorCode::OutOfOrderCall`. LibRaw refuses most of these
/// itself; `make_mem_image` is checked here, because LibRaw returns it
/// unfilled.
///
/// Neither `Send` nor `Sync`, because it holds a raw pointer. LibRaw allows
/// one instance per thread, so `Send` could be added if a caller needs it.
pub struct Processor {
    ptr: NonNull<raw::libraw_data_t>,
}

impl Processor {
    /// Allocates a LibRaw instance with no special options.
    pub fn new() -> Result<Self, Error> {
        // SAFETY: libraw_init takes no pointers and returns null or a new
        // instance that we own until libraw_close.
        let ptr = unsafe { raw::libraw_init(0) };
        NonNull::new(ptr).map(|ptr| Self { ptr }).ok_or(Error::Init)
    }

    /// Opens and identifies the file at `path`.
    pub fn open_file(&mut self, path: &Path) -> Result<(), Error> {
        let c_path = path_to_cstring(path)?;
        std::fs::File::open(path).context(OpenSnafu)?;
        // SAFETY: self.ptr is a live instance; c_path is NUL-terminated and
        // outlives the call, and LibRaw copies the name it keeps. The open
        // above keeps a missing or unreadable path away from LibRaw's
        // uninitialised-size read; a file deleted between that open and
        // LibRaw's stat can still reach it. That read gives a wrong error
        // code, not a memory access by us, and is a LibRaw bug this crate
        // cannot close without patching the vendored source. The read:
        // LibRaw_bigfile_datastream's constructor leaves _fsize unset when
        // stat fails (0.22.2 src/libraw_datastream.cpp:466).
        let rc = unsafe { raw::libraw_open_file(self.ptr.as_ptr(), c_path.as_ptr()) };
        check(rc, "libraw_open_file")
    }

    /// Decompresses the raw sensor data.
    pub fn unpack(&mut self) -> Result<(), Error> {
        // SAFETY: self.ptr is a live instance.
        let rc = unsafe { raw::libraw_unpack(self.ptr.as_ptr()) };
        check(rc, "libraw_unpack")
    }

    /// Decompresses only the embedded thumbnail. A file can have a valid
    /// thumbnail even when its raw data is corrupt.
    pub fn unpack_thumb(&mut self) -> Result<(), Error> {
        // SAFETY: self.ptr is a live instance.
        let rc = unsafe { raw::libraw_unpack_thumb(self.ptr.as_ptr()) };
        check(rc, "libraw_unpack_thumb")
    }

    /// Whether processing uses the white balance recorded at capture.
    pub fn set_use_camera_wb(&mut self, on: bool) {
        self.params().use_camera_wb = on.into();
    }

    pub fn set_output_color(&mut self, color: OutputColor) {
        self.params().output_color = match color {
            OutputColor::Raw => 0,
            OutputColor::Srgb => 1,
        };
    }

    pub fn set_output_bps(&mut self, bps: OutputBps) {
        self.params().output_bps = match bps {
            OutputBps::Eight => 8,
            OutputBps::Sixteen => 16,
        };
    }

    /// Whether each 2×2 Bayer quad becomes one output pixel instead of being
    /// interpolated into four.
    pub fn set_half_size(&mut self, on: bool) {
        self.params().half_size = on.into();
    }

    /// Demosaics and converts with the parameters set so far.
    pub fn dcraw_process(&mut self) -> Result<(), Error> {
        // SAFETY: self.ptr is a live instance.
        let rc = unsafe { raw::libraw_dcraw_process(self.ptr.as_ptr()) };
        check(rc, "libraw_dcraw_process")
    }

    /// Copies out the processed image. Fails with `OutOfOrderCall` until
    /// `dcraw_process` has run on the open file.
    pub fn make_mem_image(&mut self) -> Result<MemImage, Error> {
        // LibRaw allocates the image, then ignores the out-of-order error
        // from filling it, and returns uninitialised pixels (0.22.2
        // src/postprocessing/mem_image.cpp:301 ignores the check at 193).
        // Its own check, made here first, keeps that buffer from being read.
        if self.progress() < raw::LibRaw_progress_LIBRAW_PROGRESS_PRE_INTERPOLATE {
            return Err(Error::call(
                "libraw_dcraw_make_mem_image",
                raw::LibRaw_errors_LIBRAW_OUT_OF_ORDER_CALL,
            ));
        }
        let mut errcode = 0;
        // SAFETY: self.ptr is a live instance and errcode a valid int.
        let ptr = unsafe { raw::libraw_dcraw_make_mem_image(self.ptr.as_ptr(), &mut errcode) };
        take_mem_image(ptr, errcode, "libraw_dcraw_make_mem_image")
    }

    /// Copies out the unpacked thumbnail.
    pub fn make_mem_thumb(&mut self) -> Result<MemImage, Error> {
        let mut errcode = 0;
        // SAFETY: self.ptr is a live instance and errcode a valid int.
        let ptr = unsafe { raw::libraw_dcraw_make_mem_thumb(self.ptr.as_ptr(), &mut errcode) };
        take_mem_image(ptr, errcode, "libraw_dcraw_make_mem_thumb")
    }

    /// How far processing has gone, as LibRaw's `copy_mem_image` measures it.
    fn progress(&self) -> u32 {
        // SAFETY: self.ptr is a live instance, and `&self` rules out a
        // concurrent `&mut` borrow from `params`.
        let flags = unsafe { (*self.ptr.as_ptr()).progress_flags };
        flags & raw::LIBRAW_PROGRESS_THUMB_MASK
    }

    fn params(&mut self) -> &mut raw::libraw_output_params_t {
        // SAFETY: self.ptr is a live instance, and `&mut self` makes this
        // the only reference into it while the borrow lasts.
        unsafe { &mut (*self.ptr.as_ptr()).params }
    }
}

impl Drop for Processor {
    fn drop(&mut self) {
        // SAFETY: self.ptr came from libraw_init and is closed only here.
        unsafe { raw::libraw_close(self.ptr.as_ptr()) };
    }
}

/// Frees a `libraw_processed_image_t` on every exit path.
struct Allocation(NonNull<raw::libraw_processed_image_t>);

impl Drop for Allocation {
    fn drop(&mut self) {
        // SAFETY: the pointer came from libraw_dcraw_make_mem_image or
        // libraw_dcraw_make_mem_thumb and is freed only here.
        unsafe { raw::libraw_dcraw_clear_mem(self.0.as_ptr()) };
    }
}

/// Copies a processed image LibRaw returned into owned memory and frees it.
/// A null pointer means the call failed with `errcode`, and the error's
/// location is the caller's line.
#[track_caller]
fn take_mem_image(
    ptr: *mut raw::libraw_processed_image_t,
    errcode: i32,
    call: &'static str,
) -> Result<MemImage, Error> {
    let Some(ptr) = NonNull::new(ptr) else {
        return Err(Error::call(call, libraw_code(errcode)));
    };
    let allocation = Allocation(ptr);
    let image = allocation.0.as_ptr();

    // SAFETY: LibRaw returned a valid, initialised header, live until
    // `allocation` drops at the end of this function.
    let header = unsafe { &*image };
    // `data` is declared as one byte, but LibRaw allocates `data_size` bytes
    // from there. The pointer is taken from the allocation itself, not from a
    // reference to the one-byte array, so it may reach all of them.
    // SAFETY: as above; `data_size` bytes follow `data` in the allocation.
    let data = unsafe {
        let start = (&raw const (*image).data).cast::<u8>();
        std::slice::from_raw_parts(start, header.data_size as usize)
    }
    .to_vec();

    Ok(MemImage {
        format: ImageFormat::from_raw(header.type_),
        width: header.width.into(),
        height: header.height.into(),
        bits: header.bits,
        colors: header.colors,
        data,
    })
}

/// C's `ENOMEM`: 12 on Linux, macOS, the BSDs and the Windows CRT.
const ENOMEM: i32 = 12;

/// The `dcraw_make_mem_*` calls report a failed `malloc` as `ENOMEM`, an
/// errno, where every other failure is a `LibRaw_errors` code (0.22.2
/// `src/postprocessing/mem_image.cpp:58`, 92, 129 and 289). This puts it
/// back in LibRaw's terms.
fn libraw_code(errcode: i32) -> i32 {
    if errcode == ENOMEM {
        raw::LibRaw_errors_LIBRAW_UNSUFFICIENT_MEMORY
    } else {
        errcode
    }
}

/// `Ok` for success, else the `Call` error for `rc`, located at the caller's
/// line, so the location names the LibRaw call that failed.
#[track_caller]
fn check(rc: i32, call: &'static str) -> Result<(), Error> {
    if rc == raw::LibRaw_errors_LIBRAW_SUCCESS {
        Ok(())
    } else {
        Err(Error::call(call, rc))
    }
}

/// Unix paths are arbitrary bytes and pass through as they are; elsewhere
/// LibRaw needs UTF-8.
#[cfg(unix)]
fn path_to_cstring(path: &Path) -> Result<CString, Error> {
    use std::os::unix::ffi::OsStrExt;
    CString::new(path.as_os_str().as_bytes()).map_err(|_| Error::PathNul)
}

#[cfg(not(unix))]
fn path_to_cstring(path: &Path) -> Result<CString, Error> {
    let s = path.to_str().ok_or(Error::PathNotUtf8)?;
    CString::new(s).map_err(|_| Error::PathNul)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;

    fn call_line(err: &Error) -> u32 {
        match err {
            Error::Call { location, .. } => location.line(),
            other => panic!("not a Call: {other:?}"),
        }
    }

    #[test]
    fn a_call_error_is_located_at_the_line_that_checked_it() {
        let line = line!() + 1;
        let err = check(raw::LibRaw_errors_LIBRAW_IO_ERROR, "test").unwrap_err();
        assert_eq!(call_line(&err), line);

        let line = line!() + 1;
        let err = take_mem_image(std::ptr::null_mut(), 0, "test").unwrap_err();
        assert_eq!(call_line(&err), line);
    }

    #[test]
    fn opening_a_missing_file_fails_before_libraw() {
        let mut processor = Processor::new().unwrap();
        let err = processor
            .open_file(&std::env::temp_dir().join("does_not_exist_9f8a7b.cr2"))
            .unwrap_err();
        assert!(
            matches!(&err, Error::Open { source, .. } if source.kind() == std::io::ErrorKind::NotFound),
            "{err:?}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn opening_an_unreadable_file_fails_before_libraw() {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!(
            "rsb_libraw_ffi_unreadable_{}.cr2",
            std::process::id()
        ));
        std::fs::write(&path, b"raw bytes").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        // Root reads any file, so there is nothing to test.
        let readable = std::fs::File::open(&path).is_ok();
        let result = Processor::new().unwrap().open_file(&path);
        std::fs::remove_file(&path).ok();
        if readable {
            return;
        }
        let err = result.unwrap_err();
        assert!(
            matches!(&err, Error::Open { source, .. } if source.kind() == std::io::ErrorKind::PermissionDenied),
            "{err:?}"
        );
    }

    #[test]
    fn make_mem_image_before_processing_is_out_of_order() {
        let err = Processor::new().unwrap().make_mem_image().unwrap_err();
        assert!(
            matches!(
                err,
                Error::Call {
                    call: "libraw_dcraw_make_mem_image",
                    code: ErrorCode::OutOfOrderCall,
                    ..
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn a_failed_allocation_is_insufficient_memory() {
        let err = take_mem_image(std::ptr::null_mut(), ENOMEM, "call").unwrap_err();
        assert!(
            matches!(
                err,
                Error::Call {
                    code: ErrorCode::InsufficientMemory,
                    ..
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn libraw_reports_a_short_garbage_file_as_an_io_error() {
        let path =
            std::env::temp_dir().join(format!("rsb_libraw_ffi_not_raw_{}.cr2", std::process::id()));
        std::fs::write(&path, b"this is not a raw file at all").unwrap();
        let err = Processor::new().unwrap().open_file(&path).unwrap_err();
        std::fs::remove_file(&path).ok();
        assert!(
            matches!(
                err,
                Error::Call {
                    call: "libraw_open_file",
                    code: ErrorCode::IoError,
                    ..
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn a_path_with_a_null_byte_never_reaches_libraw() {
        use std::os::unix::ffi::OsStrExt;
        let path = Path::new(std::ffi::OsStr::from_bytes(b"/tmp/a\0b.cr2"));
        let err = Processor::new().unwrap().open_file(path).unwrap_err();
        assert!(matches!(err, Error::PathNul), "{err:?}");
    }

    #[test]
    fn image_formats_map_from_libraw() {
        assert_eq!(
            ImageFormat::from_raw(raw::LibRaw_image_formats_LIBRAW_IMAGE_JPEG),
            ImageFormat::Jpeg
        );
        assert_eq!(
            ImageFormat::from_raw(raw::LibRaw_image_formats_LIBRAW_IMAGE_BITMAP),
            ImageFormat::Bitmap
        );
        assert_eq!(ImageFormat::from_raw(99), ImageFormat::Other(99));
    }
}
