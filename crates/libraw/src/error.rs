//! The crate's error. Each variant is a failure category a caller can act
//! on; `Display` prints the whole story on one line.

use std::fmt;
use std::panic::Location;

pub use rsb_libraw_ffi::{Error as LibRawError, ErrorCode};

/// Why a decode failed. `Display` prints one line, read left to right:
/// the operation, what failed, then the category, as in
/// `LibRawAdapter::decode: libraw_unpack failed: <libraw text>: corrupt data`.
///
/// `source()` is `None`: the line already holds the cause, and a reporter
/// that walks the chain would print it twice. Code that needs the typed
/// cause, such as LibRaw's `ErrorCode`, matches the variant's `cause`.
#[derive(Debug)]
pub enum Error {
    /// The file could not be read: missing, unreadable, or a path C cannot
    /// take. Check the path and its permissions.
    Io {
        op: Option<&'static str>,
        cause: LibRawError,
        location: &'static Location<'static>,
    },
    /// The file was read, but its camera, format or thumbnail is not
    /// handled. Skip the file.
    Unsupported {
        op: Option<&'static str>,
        cause: Cause,
        location: &'static Location<'static>,
    },
    /// The file claims a supported format but its data is malformed or
    /// truncated. Reject the file.
    Corrupt {
        op: Option<&'static str>,
        cause: Cause,
        location: &'static Location<'static>,
    },
    /// LibRaw ran out of memory or hit an internal limit. Back off or abort.
    Resource {
        op: Option<&'static str>,
        cause: LibRawError,
        location: &'static Location<'static>,
    },
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

impl From<String> for Cause {
    fn from(msg: String) -> Self {
        Self::Check(msg)
    }
}

impl From<&str> for Cause {
    fn from(msg: &str) -> Self {
        Self::Check(msg.to_string())
    }
}

/// Sorts a LibRaw failure into a category. A code not listed is `Corrupt`:
/// it is safer to blame the file's data than I/O or resources. The
/// location is the `?` that converted it.
impl From<LibRawError> for Error {
    #[track_caller]
    fn from(err: LibRawError) -> Self {
        match &err {
            LibRawError::Init => Self::resource(err),
            LibRawError::Open(_) | LibRawError::PathNul => Self::io(err),
            #[cfg(not(unix))]
            LibRawError::PathNotUtf8 => Self::io(err),
            LibRawError::Call { code, .. } => match code {
                ErrorCode::IoError | ErrorCode::InputClosed => Self::io(err),
                ErrorCode::InsufficientMemory | ErrorCode::MempoolOverflow => Self::resource(err),
                ErrorCode::FileUnsupported
                | ErrorCode::UnsupportedThumbnail
                | ErrorCode::NotImplemented
                | ErrorCode::NoThumbnail
                | ErrorCode::RequestForNonexistentThumbnail => Self::unsupported(err),
                _ => Self::corrupt(err),
            },
        }
    }
}

impl Error {
    /// An `Unsupported` error, located at the caller. Takes a message, a
    /// LibRaw error or a JPEG error.
    #[track_caller]
    pub fn unsupported(cause: impl Into<Cause>) -> Self {
        Self::Unsupported {
            op: None,
            cause: cause.into(),
            location: Location::caller(),
        }
    }

    /// A `Corrupt` error, located at the caller. Takes a message, a LibRaw
    /// error or a JPEG error.
    #[track_caller]
    pub fn corrupt(cause: impl Into<Cause>) -> Self {
        Self::Corrupt {
            op: None,
            cause: cause.into(),
            location: Location::caller(),
        }
    }

    #[track_caller]
    fn io(cause: LibRawError) -> Self {
        Self::Io {
            op: None,
            cause,
            location: Location::caller(),
        }
    }

    #[track_caller]
    fn resource(cause: LibRawError) -> Self {
        Self::Resource {
            op: None,
            cause,
            location: Location::caller(),
        }
    }

    /// Names the public operation that failed, such as
    /// `LibRawAdapter::decode`. It leads the `Display` line.
    pub fn during(mut self, name: &'static str) -> Self {
        let (Self::Io { op, .. }
        | Self::Unsupported { op, .. }
        | Self::Corrupt { op, .. }
        | Self::Resource { op, .. }) = &mut self;
        *op = Some(name);
        self
    }

    /// Where the error was raised: the `?` that converted a LibRaw error,
    /// or the line that built a check error.
    pub fn location(&self) -> &'static Location<'static> {
        let (Self::Io { location, .. }
        | Self::Unsupported { location, .. }
        | Self::Corrupt { location, .. }
        | Self::Resource { location, .. }) = self;
        location
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (op, cause, category): (_, &dyn fmt::Display, _) = match self {
            Self::Io { op, cause, .. } => (op, cause, "i/o failure"),
            Self::Unsupported { op, cause, .. } => (op, cause, "unsupported format"),
            Self::Corrupt { op, cause, .. } => (op, cause, "corrupt data"),
            Self::Resource { op, cause, .. } => (op, cause, "resource exhaustion"),
        };
        if let Some(op) = op {
            write!(f, "{op}: ")?;
        }
        write!(f, "{cause}: {category}")
    }
}

impl std::error::Error for Error {}
