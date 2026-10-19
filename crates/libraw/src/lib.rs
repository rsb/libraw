mod adapter;
pub mod decode;
pub mod error;
pub mod image;

pub use adapter::LibRawAdapter;
pub use decode::RawDecode;
pub use error::Error;
pub use image::ImageBuffer;
/// The LibRaw layer, so a caller can match `Error`'s sources, such as
/// `ffi::Error::Call { code, .. }`, without depending on it directly.
pub use rsb_libraw_ffi as ffi;
