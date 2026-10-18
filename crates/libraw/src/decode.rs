use std::path::Path;

use crate::error::{Error, NoPreviewSnafu, NoThumbnailSnafu};
use crate::image::ImageBuffer;

/// The abstraction boundary for RAW decoding. Any backend (LibRaw today, a
/// pure-Rust decoder or a mock tomorrow) implements this trait, and calling
/// code programs against the trait without knowing which decoder is behind it.
///
/// `Send + Sync` bounds allow implementors to be shared across threads (e.g.,
/// behind an `Arc` in a thread pool). `LibRawAdapter` satisfies both trivially
/// as a zero-sized type.
///
/// Takes `&self` rather than `&mut self` — decoding is read-only from the
/// caller's perspective, so multiple threads can call decode on the same
/// adapter concurrently without a mutex.
///
/// Each method is stateless and self-contained: path in, image out. There is
/// no open/close lifecycle, which eliminates use-after-close and wrong-order
/// bugs at the cost of opening the file once per method call.
pub trait RawDecode: Send + Sync {
    /// Full-resolution decode. The only required method — the minimum contract
    /// for being a decoder. Takes a file path (not a reader) because LibRaw's
    /// C API operates on paths, and it avoids forcing 25-80MB RAW files into
    /// memory before decoding begins.
    fn decode(&self, path: &Path) -> Result<ImageBuffer, Error>;

    /// Extracts the camera-generated thumbnail. Default returns
    /// `NoThumbnail` so implementors that lack thumbnail support don't need
    /// boilerplate.
    fn decode_thumbnail(&self, path: &Path) -> Result<ImageBuffer, Error> {
        NoThumbnailSnafu {
            op: "decode_thumbnail",
            path,
        }
        .fail()
    }

    /// Half-resolution decode for fast previews. Default returns `NoPreview`,
    /// as decode_thumbnail does.
    fn decode_preview(&self, path: &Path) -> Result<ImageBuffer, Error> {
        NoPreviewSnafu {
            op: "decode_preview",
            path,
        }
        .fail()
    }
}
