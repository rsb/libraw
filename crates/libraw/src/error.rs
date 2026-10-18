use std::path::PathBuf;

use rsb_errors::Loc;
use rsb_libraw_ffi::{self as ffi, ErrorCode};
use snafu::prelude::*;

// ---------------------------------------------------------------------------
// Kind — the four failure categories that downstream code can match on
// ---------------------------------------------------------------------------

/// Classifies every error into one of four categories so callers can decide
/// how to respond without parsing message strings. Derived from the variant
/// and, for LibRaw failures, from LibRaw's code; see [`Error::kind`].
///
/// - `Io` — the file could not be read at all (missing, permissions, device).
/// - `Unsupported` — the file was read but its format is not handled.
/// - `Corrupt` — the file claims to be a supported format but its data is
///   malformed, truncated, or internally inconsistent.
/// - `Resource` — the operation failed due to memory or system limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Io,
    Unsupported,
    Corrupt,
    Resource,
}

// ---------------------------------------------------------------------------
// Error — one variant per failure the crate reports
// ---------------------------------------------------------------------------

/// The crate's single error type. Every fallible operation returns this.
///
/// Each variant names one failure. `op` is the public method that failed
/// (`decode`, `decode_preview`, `decode_thumbnail`) and `path` the file it
/// was given, so `{e}` reads as one line from the method to LibRaw's text:
/// `decode /photos/DSCF0001.RAF: open: cannot open file: No such file or
/// directory (os error 2)`, with ` (at file:line)` after each wrapping step
/// unless `rsb_errors::show_locations(false)`.
///
/// `Send + Sync`, so it can cross thread boundaries.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// LibRaw could not create its state.
    #[snafu(display("{op} {}: init{location}: {source}", path.display()))]
    Init {
        op: &'static str,
        path: PathBuf,
        source: ffi::Error,
        #[snafu(implicit)]
        location: Loc,
    },
    /// The file could not be opened or identified. `exists` is whether the
    /// path existed when the open failed: LibRaw reports a file too short to
    /// identify as an I/O error, and an existing file makes that `Corrupt`.
    #[snafu(display("{op} {}: open{location}: {source}", path.display()))]
    Open {
        op: &'static str,
        path: PathBuf,
        exists: bool,
        source: ffi::Error,
        #[snafu(implicit)]
        location: Loc,
    },
    /// LibRaw could not unpack the raw data.
    #[snafu(display("{op} {}: unpack{location}: {source}", path.display()))]
    Unpack {
        op: &'static str,
        path: PathBuf,
        source: ffi::Error,
        #[snafu(implicit)]
        location: Loc,
    },
    /// LibRaw could not unpack the embedded thumbnail.
    #[snafu(display("{op} {}: unpack thumbnail{location}: {source}", path.display()))]
    UnpackThumb {
        op: &'static str,
        path: PathBuf,
        source: ffi::Error,
        #[snafu(implicit)]
        location: Loc,
    },
    /// LibRaw could not demosaic and process the raw data.
    #[snafu(display("{op} {}: process{location}: {source}", path.display()))]
    Process {
        op: &'static str,
        path: PathBuf,
        source: ffi::Error,
        #[snafu(implicit)]
        location: Loc,
    },
    /// LibRaw could not hand over the processed image.
    #[snafu(display("{op} {}: make image{location}: {source}", path.display()))]
    MakeImage {
        op: &'static str,
        path: PathBuf,
        source: ffi::Error,
        #[snafu(implicit)]
        location: Loc,
    },
    /// LibRaw could not hand over the thumbnail.
    #[snafu(display("{op} {}: make thumbnail{location}: {source}", path.display()))]
    MakeThumb {
        op: &'static str,
        path: PathBuf,
        source: ffi::Error,
        #[snafu(implicit)]
        location: Loc,
    },
    /// The embedded JPEG thumbnail could not be decoded.
    #[snafu(display("{op} {}: decode JPEG thumbnail{location}: {source}", path.display()))]
    Jpeg {
        op: &'static str,
        path: PathBuf,
        source: jpeg_decoder::Error,
        #[snafu(implicit)]
        location: Loc,
    },
    /// The pixels LibRaw or the JPEG decoder produced do not fit the image
    /// they claim to be. The source is a [`Error::Dimensions`].
    #[snafu(display("{op} {}: convert pixels{location}: {source}", path.display()))]
    Convert {
        op: &'static str,
        path: PathBuf,
        #[snafu(source(from(Error, Box::new)))]
        source: Box<Error>,
        #[snafu(implicit)]
        location: Loc,
    },
    /// The processed image is not the 16-bit RGB bitmap this crate asked for.
    #[snafu(display("{op} {}: output format: {what}", path.display()))]
    Format {
        op: &'static str,
        path: PathBuf,
        what: String,
    },
    /// The thumbnail is in a format this crate does not convert.
    #[snafu(display("{op} {}: thumbnail format: {what}", path.display()))]
    ThumbnailFormat {
        op: &'static str,
        path: PathBuf,
        what: String,
    },
    /// Pixel data does not match the width and height given with it.
    /// Returned by the `ImageBuffer` constructors, which know no path.
    #[snafu(display("image dimensions: {what}"))]
    Dimensions { what: String },
    /// The decoder does not extract thumbnails: `RawDecode`'s default.
    #[snafu(display("{op} {}: thumbnails not supported by this decoder", path.display()))]
    NoThumbnail { op: &'static str, path: PathBuf },
    /// The decoder does not make previews: `RawDecode`'s default.
    #[snafu(display("{op} {}: previews not supported by this decoder", path.display()))]
    NoPreview { op: &'static str, path: PathBuf },
}

impl Error {
    /// The category a caller branches on. LibRaw failures take theirs from
    /// LibRaw's code; an unrecognised code is `Corrupt`, since bad data is
    /// a safer guess than I/O or resources.
    pub fn kind(&self) -> Kind {
        match self {
            Error::Open {
                exists: true,
                source:
                    ffi::Error::Call {
                        code: ErrorCode::IoError,
                        ..
                    },
                ..
            } => Kind::Corrupt,
            Error::Init { source, .. }
            | Error::Open { source, .. }
            | Error::Unpack { source, .. }
            | Error::UnpackThumb { source, .. }
            | Error::Process { source, .. }
            | Error::MakeImage { source, .. }
            | Error::MakeThumb { source, .. } => ffi_kind(source),
            Error::Convert { source, .. } => source.kind(),
            Error::Jpeg { .. } | Error::Dimensions { .. } => Kind::Corrupt,
            Error::Format { .. }
            | Error::ThumbnailFormat { .. }
            | Error::NoThumbnail { .. }
            | Error::NoPreview { .. } => Kind::Unsupported,
        }
    }

    /// The variant's path, dotted and prefixed through each layer. Codes are
    /// API: add one, never rename one.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Init { .. } => "libraw.init.init",
            Error::Open { source, .. } => match source {
                ffi::Error::Open { .. } => "libraw.open.open",
                ffi::Error::PathNul => "libraw.open.path_nul",
                #[cfg(not(unix))]
                ffi::Error::PathNotUtf8 => "libraw.open.path_not_utf8",
                ffi::Error::Call { .. } => "libraw.open.call",
                ffi::Error::Init => "libraw.open.init",
            },
            Error::Unpack { .. } => "libraw.unpack.call",
            Error::UnpackThumb { .. } => "libraw.unpack_thumb.call",
            Error::Process { .. } => "libraw.process.call",
            Error::MakeImage { .. } => "libraw.make_image.call",
            Error::MakeThumb { .. } => "libraw.make_thumb.call",
            Error::Jpeg { .. } => "libraw.jpeg",
            Error::Convert { .. } => "libraw.convert.dimensions",
            Error::Format { .. } => "libraw.format",
            Error::ThumbnailFormat { .. } => "libraw.thumbnail_format",
            Error::Dimensions { .. } => "libraw.dimensions",
            Error::NoThumbnail { .. } => "libraw.no_thumbnail",
            Error::NoPreview { .. } => "libraw.no_preview",
        }
    }
}

fn ffi_kind(err: &ffi::Error) -> Kind {
    match err {
        ffi::Error::Init => Kind::Resource,
        ffi::Error::Open { .. } | ffi::Error::PathNul => Kind::Io,
        #[cfg(not(unix))]
        ffi::Error::PathNotUtf8 => Kind::Io,
        ffi::Error::Call { code, .. } => match code {
            ErrorCode::IoError | ErrorCode::InputClosed => Kind::Io,
            ErrorCode::InsufficientMemory | ErrorCode::MempoolOverflow => Kind::Resource,
            ErrorCode::FileUnsupported
            | ErrorCode::UnsupportedThumbnail
            | ErrorCode::NotImplemented
            | ErrorCode::NoThumbnail
            | ErrorCode::RequestForNonexistentThumbnail => Kind::Unsupported,
            _ => Kind::Corrupt,
        },
    }
}
