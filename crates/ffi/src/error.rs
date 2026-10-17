use std::ffi::{CStr, c_char};

use crate::bindings as raw;

/// A LibRaw return code: one variant per `LibRaw_errors` constant in
/// `libraw_const.h`. `Other` carries a code this LibRaw version does not
/// define, so no code is lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    Unspecified,
    FileUnsupported,
    RequestForNonexistentImage,
    OutOfOrderCall,
    NoThumbnail,
    UnsupportedThumbnail,
    InputClosed,
    NotImplemented,
    RequestForNonexistentThumbnail,
    InsufficientMemory,
    DataError,
    IoError,
    CancelledByCallback,
    BadCrop,
    TooBig,
    MempoolOverflow,
    Other(i32),
}

impl ErrorCode {
    pub(crate) fn from_raw(rc: i32) -> Self {
        match rc {
            raw::LibRaw_errors_LIBRAW_UNSPECIFIED_ERROR => Self::Unspecified,
            raw::LibRaw_errors_LIBRAW_FILE_UNSUPPORTED => Self::FileUnsupported,
            raw::LibRaw_errors_LIBRAW_REQUEST_FOR_NONEXISTENT_IMAGE => {
                Self::RequestForNonexistentImage
            }
            raw::LibRaw_errors_LIBRAW_OUT_OF_ORDER_CALL => Self::OutOfOrderCall,
            raw::LibRaw_errors_LIBRAW_NO_THUMBNAIL => Self::NoThumbnail,
            raw::LibRaw_errors_LIBRAW_UNSUPPORTED_THUMBNAIL => Self::UnsupportedThumbnail,
            raw::LibRaw_errors_LIBRAW_INPUT_CLOSED => Self::InputClosed,
            raw::LibRaw_errors_LIBRAW_NOT_IMPLEMENTED => Self::NotImplemented,
            raw::LibRaw_errors_LIBRAW_REQUEST_FOR_NONEXISTENT_THUMBNAIL => {
                Self::RequestForNonexistentThumbnail
            }
            raw::LibRaw_errors_LIBRAW_UNSUFFICIENT_MEMORY => Self::InsufficientMemory,
            raw::LibRaw_errors_LIBRAW_DATA_ERROR => Self::DataError,
            raw::LibRaw_errors_LIBRAW_IO_ERROR => Self::IoError,
            raw::LibRaw_errors_LIBRAW_CANCELLED_BY_CALLBACK => Self::CancelledByCallback,
            raw::LibRaw_errors_LIBRAW_BAD_CROP => Self::BadCrop,
            raw::LibRaw_errors_LIBRAW_TOO_BIG => Self::TooBig,
            raw::LibRaw_errors_LIBRAW_MEMPOOL_OVERFLOW => Self::MempoolOverflow,
            other => Self::Other(other),
        }
    }
}

/// Why a `Processor` call failed. Says what happened at the C boundary;
/// what the caller should do about it is `rsb-libraw`'s decision.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// `libraw_init` returned null: LibRaw could not allocate its state.
    #[error("libraw_init returned null")]
    Init,
    /// `stat` on the path failed, checked before LibRaw sees it. LibRaw's own
    /// open reads an uninitialised size when its `stat` fails, and reports a
    /// missing file as `TooBig` or `IoError` at random. Read permission is
    /// not checked here; LibRaw reports that as `IoError`.
    #[error("cannot open file: {0}")]
    Open(std::io::Error),
    /// The path holds a null byte, so C cannot see all of it.
    #[error("path contains interior null byte")]
    PathNul,
    /// The path is not UTF-8, which LibRaw needs outside Unix.
    #[cfg(not(unix))]
    #[error("path is not valid UTF-8")]
    PathNotUtf8,
    /// A LibRaw call returned a code other than success. `message` is
    /// LibRaw's own text for the code.
    #[error("{call} failed: {message}")]
    Call {
        call: &'static str,
        code: ErrorCode,
        message: String,
    },
}

impl Error {
    /// The error for LibRaw call `call` returning `rc`, with LibRaw's text
    /// for the code.
    pub(crate) fn call(call: &'static str, rc: i32) -> Self {
        // SAFETY: libraw_strerror takes any int and returns null or a
        // pointer to a static NUL-terminated string.
        let text = unsafe { raw::libraw_strerror(rc) };
        // SAFETY: as above, `text` is null or a static C string.
        let message = unsafe { strerror_message(text, rc) };
        Self::Call {
            call,
            code: ErrorCode::from_raw(rc),
            message,
        }
    }
}

/// The text behind a `libraw_strerror` pointer, or the bare code when the
/// pointer is null.
///
/// # Safety
///
/// `text` is null or points to a NUL-terminated string that outlives the
/// call.
unsafe fn strerror_message(text: *const c_char, rc: i32) -> String {
    if text.is_null() {
        format!("code {rc}")
    } else {
        // SAFETY: the caller guarantees a live NUL-terminated string.
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use super::*;

    #[test]
    fn each_libraw_error_constant_has_its_own_code() {
        let cases = [
            (
                raw::LibRaw_errors_LIBRAW_UNSPECIFIED_ERROR,
                ErrorCode::Unspecified,
            ),
            (
                raw::LibRaw_errors_LIBRAW_FILE_UNSUPPORTED,
                ErrorCode::FileUnsupported,
            ),
            (
                raw::LibRaw_errors_LIBRAW_REQUEST_FOR_NONEXISTENT_IMAGE,
                ErrorCode::RequestForNonexistentImage,
            ),
            (
                raw::LibRaw_errors_LIBRAW_OUT_OF_ORDER_CALL,
                ErrorCode::OutOfOrderCall,
            ),
            (
                raw::LibRaw_errors_LIBRAW_NO_THUMBNAIL,
                ErrorCode::NoThumbnail,
            ),
            (
                raw::LibRaw_errors_LIBRAW_UNSUPPORTED_THUMBNAIL,
                ErrorCode::UnsupportedThumbnail,
            ),
            (
                raw::LibRaw_errors_LIBRAW_INPUT_CLOSED,
                ErrorCode::InputClosed,
            ),
            (
                raw::LibRaw_errors_LIBRAW_NOT_IMPLEMENTED,
                ErrorCode::NotImplemented,
            ),
            (
                raw::LibRaw_errors_LIBRAW_REQUEST_FOR_NONEXISTENT_THUMBNAIL,
                ErrorCode::RequestForNonexistentThumbnail,
            ),
            (
                raw::LibRaw_errors_LIBRAW_UNSUFFICIENT_MEMORY,
                ErrorCode::InsufficientMemory,
            ),
            (raw::LibRaw_errors_LIBRAW_DATA_ERROR, ErrorCode::DataError),
            (raw::LibRaw_errors_LIBRAW_IO_ERROR, ErrorCode::IoError),
            (
                raw::LibRaw_errors_LIBRAW_CANCELLED_BY_CALLBACK,
                ErrorCode::CancelledByCallback,
            ),
            (raw::LibRaw_errors_LIBRAW_BAD_CROP, ErrorCode::BadCrop),
            (raw::LibRaw_errors_LIBRAW_TOO_BIG, ErrorCode::TooBig),
            (
                raw::LibRaw_errors_LIBRAW_MEMPOOL_OVERFLOW,
                ErrorCode::MempoolOverflow,
            ),
        ];
        for (rc, code) in cases {
            assert_eq!(ErrorCode::from_raw(rc), code, "rc {rc}");
        }
    }

    #[test]
    fn an_unknown_code_is_kept() {
        assert_eq!(ErrorCode::from_raw(999999), ErrorCode::Other(999999));
    }

    #[test]
    fn a_call_error_names_the_call_and_libraw_text() {
        let err = Error::call("libraw_open_file", raw::LibRaw_errors_LIBRAW_IO_ERROR);
        assert!(matches!(
            err,
            Error::Call {
                call: "libraw_open_file",
                code: ErrorCode::IoError,
                ..
            }
        ));
        assert!(err.to_string().starts_with("libraw_open_file failed: "));
    }

    #[test]
    fn strerror_message_null_falls_back_to_the_code() {
        // SAFETY: null is allowed.
        let message = unsafe { strerror_message(std::ptr::null(), 12345) };
        assert_eq!(message, "code 12345");
    }

    #[test]
    fn strerror_message_uses_libraw_text() {
        let text = CString::new("test error message").unwrap();
        // SAFETY: `text` is a live C string.
        let message = unsafe { strerror_message(text.as_ptr(), 1) };
        assert_eq!(message, "test error message");
    }
}
