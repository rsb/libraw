//! The crate's error. Each variant is a failure category a caller can act
//! on; the detail is in `source()`.

pub use rsb_libraw_ffi::{Error as LibRawError, ErrorCode};

/// Why a decode failed. `Display` names the category only; walk
/// `source()` for what LibRaw, the JPEG decoder or this crate found.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The file could not be read: missing, unreadable, or a path C cannot
    /// take. Check the path and its permissions.
    #[error("i/o failure")]
    Io(#[source] LibRawError),
    /// The file was read, but its camera, format or thumbnail is not
    /// handled. Skip the file.
    #[error("unsupported format")]
    Unsupported(#[source] Cause),
    /// The file claims a supported format but its data is malformed or
    /// truncated. Reject the file.
    #[error("corrupt data")]
    Corrupt(#[source] Cause),
    /// LibRaw ran out of memory or hit an internal limit. Back off or abort.
    #[error("resource exhaustion")]
    Resource(#[source] LibRawError),
}

/// What found an `Unsupported` or `Corrupt` problem.
#[derive(Debug, thiserror::Error)]
pub enum Cause {
    #[error(transparent)]
    LibRaw(#[from] LibRawError),
    #[error("JPEG thumbnail decode failed: {0}")]
    Jpeg(#[from] jpeg_decoder::Error),
    /// A check in this crate, such as image dimensions or output format.
    #[error("{0}")]
    Check(String),
}

/// Sorts a LibRaw failure into a category. A code not listed is `Corrupt`:
/// it is safer to blame the file's data than I/O or resources.
impl From<LibRawError> for Error {
    fn from(err: LibRawError) -> Self {
        match &err {
            LibRawError::Init => Self::Resource(err),
            LibRawError::Open(_) | LibRawError::PathNul => Self::Io(err),
            #[cfg(not(unix))]
            LibRawError::PathNotUtf8 => Self::Io(err),
            LibRawError::Call { code, .. } => match code {
                ErrorCode::IoError | ErrorCode::InputClosed => Self::Io(err),
                ErrorCode::InsufficientMemory | ErrorCode::MempoolOverflow => Self::Resource(err),
                ErrorCode::FileUnsupported
                | ErrorCode::UnsupportedThumbnail
                | ErrorCode::NotImplemented
                | ErrorCode::NoThumbnail
                | ErrorCode::RequestForNonexistentThumbnail => Self::Unsupported(err.into()),
                _ => Self::Corrupt(err.into()),
            },
        }
    }
}

impl Error {
    /// A `Corrupt` error from a check in this crate.
    pub(crate) fn corrupt(msg: impl Into<String>) -> Self {
        Self::Corrupt(Cause::Check(msg.into()))
    }

    /// An `Unsupported` error from a check in this crate.
    pub(crate) fn unsupported(msg: impl Into<String>) -> Self {
        Self::Unsupported(Cause::Check(msg.into()))
    }
}
