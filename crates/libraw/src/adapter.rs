use std::path::Path;

use rsb_libraw_ffi::{self as ffi, ErrorCode, ImageFormat, MemImage, OutputBps, OutputColor};

use crate::decode::RawDecode;
use crate::error::{Cause, Error};
use crate::image::ImageBuffer;

// ---------------------------------------------------------------------------
// LibRawAdapter — the public RawDecode implementation
// ---------------------------------------------------------------------------

/// A zero-sized strategy type that implements `RawDecode` via LibRaw.
/// Carries no state — a `Processor` is created and dropped within each
/// method call. Exists as a struct so it can be used polymorphically via
/// `Box<dyn RawDecode>`, enabling backend-swappable decoding.
pub struct LibRawAdapter;

impl RawDecode for LibRawAdapter {
    /// Full-resolution decode: open → unpack → configure sRGB 16-bit →
    /// demosaic/process → extract image.
    fn decode(&self, path: &Path) -> Result<ImageBuffer, Error> {
        let mut processor = ffi::Processor::new()?;
        open_file(&mut processor, path)?;
        processor.unpack()?;
        configure_output_srgb16(&mut processor);
        processor.dcraw_process()?;
        rgb16_image(processor.make_mem_image()?)
    }

    /// Extracts the camera-generated thumbnail. No processing step — thumbnails
    /// are pre-rendered by the camera firmware.
    fn decode_thumbnail(&self, path: &Path) -> Result<ImageBuffer, Error> {
        let mut processor = ffi::Processor::new()?;
        open_file(&mut processor, path)?;
        processor.unpack_thumb()?;
        thumbnail_image(processor.make_mem_thumb()?)
    }

    /// Half-resolution decode for fast previews. Same pipeline as decode but
    /// with half_size enabled — each Bayer quad becomes one pixel instead of
    /// four, trading resolution for speed.
    fn decode_preview(&self, path: &Path) -> Result<ImageBuffer, Error> {
        let mut processor = ffi::Processor::new()?;
        open_file(&mut processor, path)?;
        processor.unpack()?;
        processor.set_half_size(true);
        configure_output_srgb16(&mut processor);
        processor.dcraw_process()?;
        rgb16_image(processor.make_mem_image()?)
    }
}

// ---------------------------------------------------------------------------
// Processing policy
// ---------------------------------------------------------------------------

/// Configures the processing pipeline for full-quality display output:
/// - camera white balance: honor the white balance recorded at capture time
/// - sRGB output: what monitors expect
/// - 16 bits per sample: preserve full dynamic range through processing;
///   the downsampling to 8-bit happens later in ImageBuffer::from_rgb16
fn configure_output_srgb16(processor: &mut ffi::Processor) {
    processor.set_use_camera_wb(true);
    processor.set_output_color(OutputColor::Srgb);
    processor.set_output_bps(OutputBps::Sixteen);
}

/// Opens the file, applying the open-file classification below.
fn open_file(processor: &mut ffi::Processor, path: &Path) -> Result<(), Error> {
    processor
        .open_file(path)
        .map_err(|e| classify_open_file_error(e, path.exists()))
}

/// Converts a processed full image. Unlike thumbnails, the output format is
/// strictly validated: we configured 16-bit 3-channel sRGB, so anything else
/// indicates a LibRaw bug or a pathological image.
fn rgb16_image(image: MemImage) -> Result<ImageBuffer, Error> {
    if image.format != ImageFormat::Bitmap {
        return Err(Error::unsupported("decoded image is not bitmap format"));
    }
    if image.bits != 16 {
        return Err(Error::unsupported(format!(
            "expected 16-bit output, got {}-bit",
            image.bits
        )));
    }
    if image.colors != 3 {
        return Err(Error::unsupported(format!(
            "expected 3-channel RGB, got {} channels",
            image.colors
        )));
    }
    ImageBuffer::from_rgb16(&image.data, image.width, image.height)
}

/// Converts an extracted thumbnail. Thumbnails can be JPEG (most cameras),
/// 8-bit bitmap, or 16-bit bitmap — this handles all three and returns
/// Unsupported for anything else.
fn thumbnail_image(image: MemImage) -> Result<ImageBuffer, Error> {
    let MemImage {
        format,
        width,
        height,
        bits,
        colors,
        data,
    } = image;
    match format {
        ImageFormat::Jpeg => decode_jpeg_thumbnail(&data),
        ImageFormat::Bitmap if bits == 8 && colors == 3 => {
            ImageBuffer::from_rgb8(&data, width, height)
        }
        ImageFormat::Bitmap if bits == 16 && colors == 3 => {
            ImageBuffer::from_rgb16(&data, width, height)
        }
        ImageFormat::Bitmap => Err(Error::unsupported(format!(
            "thumbnail bitmap format: {bits}-bit, {colors} channels"
        ))),
        other => Err(Error::unsupported(format!("thumbnail format {other:?}"))),
    }
}

/// Decodes an embedded JPEG thumbnail into an ImageBuffer. Most cameras store
/// thumbnails as JPEGs; this normalizes them into the same pixel format as
/// full decodes so downstream code doesn't need to know the original encoding.
fn decode_jpeg_thumbnail(jpeg_data: &[u8]) -> Result<ImageBuffer, Error> {
    use jpeg_decoder::{Decoder, PixelFormat};

    let mut decoder = Decoder::new(jpeg_data);
    let pixels = decoder
        .decode()
        .map_err(|e| Error::Corrupt(Cause::Jpeg(e)))?;

    let info = decoder
        .info()
        .ok_or_else(|| Error::corrupt("JPEG thumbnail decoded but produced no image info"))?;

    match info.pixel_format {
        PixelFormat::RGB24 => {
            ImageBuffer::from_rgb8(&pixels, info.width as u32, info.height as u32)
        }
        // Grayscale thumbnails (some older cameras): expand each gray value to R=G=B.
        PixelFormat::L8 => {
            let rgb: Vec<u8> = pixels.iter().flat_map(|&g| [g, g, g]).collect();
            ImageBuffer::from_rgb8(&rgb, info.width as u32, info.height as u32)
        }
        other => Err(Error::unsupported(format!(
            "JPEG thumbnail pixel format: {other:?}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Error classification
// ---------------------------------------------------------------------------

/// TOCTOU heuristic for open_file failures. `Processor::open_file` has
/// already opened the file for reading, so if LibRaw then reports an I/O
/// error and the file still exists, the error is reclassified as Corrupt:
/// LibRaw reports a file too short to identify this way. The race condition
/// (file deleted between LibRaw's open and our exists check) is accepted as
/// best-effort.
fn classify_open_file_error(err: ffi::Error, path_exists: bool) -> Error {
    match err {
        ffi::Error::Call {
            code: ErrorCode::IoError,
            ..
        } if path_exists => Error::Corrupt(Cause::LibRaw(err)),
        other => other.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(code: ErrorCode) -> ffi::Error {
        ffi::Error::Call {
            call: "test",
            code,
            message: String::new(),
        }
    }

    #[test]
    fn decode_jpeg_thumbnail_corrupt_data() {
        let result = decode_jpeg_thumbnail(b"not a jpeg at all");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::Corrupt(Cause::Jpeg(_))
        ));
    }

    #[test]
    fn decode_jpeg_thumbnail_empty_data() {
        let result = decode_jpeg_thumbnail(b"");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::Corrupt(Cause::Jpeg(_))
        ));
    }

    #[test]
    fn classify_open_file_io_error_with_existing_path_returns_corrupt() {
        let err = classify_open_file_error(call(ErrorCode::IoError), true);
        assert!(matches!(err, Error::Corrupt(Cause::LibRaw(_))));
    }

    #[test]
    fn classify_open_file_io_error_with_missing_path_returns_io() {
        let err = classify_open_file_error(call(ErrorCode::IoError), false);
        assert!(matches!(err, Error::Io(_)));
    }

    #[test]
    fn classify_open_file_non_io_error_ignores_path_exists() {
        let err = classify_open_file_error(call(ErrorCode::FileUnsupported), true);
        assert!(matches!(err, Error::Unsupported(_)));
    }
}
