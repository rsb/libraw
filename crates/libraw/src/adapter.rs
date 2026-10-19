use std::path::Path;

use rsb_libraw_ffi::{self as ffi, ImageFormat, MemImage, OutputBps, OutputColor};
use snafu::prelude::*;

use crate::decode::RawDecode;
use crate::error::{
    ConvertSnafu, Error, FormatSnafu, InitSnafu, JpegSnafu, MakeImageSnafu, MakeThumbSnafu,
    OpenSnafu, ProcessSnafu, ThumbnailFormatSnafu, UnpackSnafu, UnpackThumbSnafu,
};
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
        let op = "decode";
        let mut processor = ffi::Processor::new().context(InitSnafu { op, path })?;
        open_file(&mut processor, op, path)?;
        processor.unpack().context(UnpackSnafu { op, path })?;
        configure_output_srgb16(&mut processor);
        processor
            .dcraw_process()
            .context(ProcessSnafu { op, path })?;
        let image = processor
            .make_mem_image()
            .context(MakeImageSnafu { op, path })?;
        rgb16_image(image, op, path)
    }

    /// Extracts the camera-generated thumbnail. No processing step — thumbnails
    /// are pre-rendered by the camera firmware.
    fn decode_thumbnail(&self, path: &Path) -> Result<ImageBuffer, Error> {
        let op = "decode_thumbnail";
        let mut processor = ffi::Processor::new().context(InitSnafu { op, path })?;
        open_file(&mut processor, op, path)?;
        processor
            .unpack_thumb()
            .context(UnpackThumbSnafu { op, path })?;
        let image = processor
            .make_mem_thumb()
            .context(MakeThumbSnafu { op, path })?;
        thumbnail_image(image, op, path)
    }

    /// Half-resolution decode for fast previews. Same pipeline as decode but
    /// with half_size enabled — each Bayer quad becomes one pixel instead of
    /// four, trading resolution for speed.
    fn decode_preview(&self, path: &Path) -> Result<ImageBuffer, Error> {
        let op = "decode_preview";
        let mut processor = ffi::Processor::new().context(InitSnafu { op, path })?;
        open_file(&mut processor, op, path)?;
        processor.unpack().context(UnpackSnafu { op, path })?;
        processor.set_half_size(true);
        configure_output_srgb16(&mut processor);
        processor
            .dcraw_process()
            .context(ProcessSnafu { op, path })?;
        let image = processor
            .make_mem_image()
            .context(MakeImageSnafu { op, path })?;
        rgb16_image(image, op, path)
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

/// Opens the file, recording whether the path exists for `Error::kind`.
/// `Processor::open_file` has already opened the file for reading, so an
/// I/O error from LibRaw on a file that still exists means the file is too
/// short to identify. The race (the file deleted between LibRaw's open and
/// the `exists` check) is accepted as best-effort.
#[track_caller]
fn open_file(processor: &mut ffi::Processor, op: &'static str, path: &Path) -> Result<(), Error> {
    let result = processor.open_file(path);
    let exists = result.is_err() && path.exists();
    result.context(OpenSnafu { op, path, exists })
}

/// Converts a processed full image. Unlike thumbnails, the output format is
/// strictly validated: we configured 16-bit 3-channel sRGB, so anything else
/// indicates a LibRaw bug or a pathological image.
fn rgb16_image(image: MemImage, op: &'static str, path: &Path) -> Result<ImageBuffer, Error> {
    ensure!(
        image.format == ImageFormat::Bitmap,
        FormatSnafu {
            op,
            path,
            what: format!("{:?}, not a bitmap", image.format),
        }
    );
    ensure!(
        image.bits == 16,
        FormatSnafu {
            op,
            path,
            what: format!("{}-bit, not 16-bit", image.bits),
        }
    );
    ensure!(
        image.colors == 3,
        FormatSnafu {
            op,
            path,
            what: format!("{} channels, not 3", image.colors),
        }
    );
    ImageBuffer::from_rgb16(&image.data, image.width, image.height)
        .context(ConvertSnafu { op, path })
}

/// Converts an extracted thumbnail. Thumbnails can be JPEG (most cameras),
/// 8-bit bitmap, or 16-bit bitmap — this handles all three and returns
/// `ThumbnailFormat` for anything else.
fn thumbnail_image(image: MemImage, op: &'static str, path: &Path) -> Result<ImageBuffer, Error> {
    let MemImage {
        format,
        width,
        height,
        bits,
        colors,
        data,
    } = image;
    match format {
        ImageFormat::Jpeg => decode_jpeg_thumbnail(&data, op, path),
        ImageFormat::Bitmap if bits == 8 && colors == 3 => {
            ImageBuffer::from_rgb8(&data, width, height).context(ConvertSnafu { op, path })
        }
        ImageFormat::Bitmap if bits == 16 && colors == 3 => {
            ImageBuffer::from_rgb16(&data, width, height).context(ConvertSnafu { op, path })
        }
        ImageFormat::Bitmap => ThumbnailFormatSnafu {
            op,
            path,
            what: format!("{bits}-bit bitmap with {colors} channels"),
        }
        .fail(),
        other => ThumbnailFormatSnafu {
            op,
            path,
            what: format!("{other:?}"),
        }
        .fail(),
    }
}

/// Decodes an embedded JPEG thumbnail into an ImageBuffer. Most cameras store
/// thumbnails as JPEGs; this normalizes them into the same pixel format as
/// full decodes so downstream code doesn't need to know the original encoding.
fn decode_jpeg_thumbnail(
    jpeg_data: &[u8],
    op: &'static str,
    path: &Path,
) -> Result<ImageBuffer, Error> {
    use jpeg_decoder::{Decoder, PixelFormat};

    let mut decoder = Decoder::new(jpeg_data);
    let pixels = decoder.decode().context(JpegSnafu { op, path })?;
    let info = decoder.info().context(ThumbnailFormatSnafu {
        op,
        path,
        what: "JPEG with no image info",
    })?;

    match info.pixel_format {
        PixelFormat::RGB24 => {
            ImageBuffer::from_rgb8(&pixels, info.width as u32, info.height as u32)
                .context(ConvertSnafu { op, path })
        }
        // Grayscale thumbnails (some older cameras): expand each gray value to R=G=B.
        PixelFormat::L8 => {
            let rgb: Vec<u8> = pixels.iter().flat_map(|&g| [g, g, g]).collect();
            ImageBuffer::from_rgb8(&rgb, info.width as u32, info.height as u32)
                .context(ConvertSnafu { op, path })
        }
        other => ThumbnailFormatSnafu {
            op,
            path,
            what: format!("JPEG in {other:?}"),
        }
        .fail(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Kind;

    #[test]
    fn decode_jpeg_thumbnail_corrupt_data() {
        let err = decode_jpeg_thumbnail(b"not a jpeg at all", "decode_thumbnail", Path::new("x"))
            .unwrap_err();
        assert!(matches!(err, Error::Jpeg { .. }), "{err:?}");
        assert_eq!(err.kind(), Kind::Corrupt);
    }

    #[test]
    fn decode_jpeg_thumbnail_empty_data() {
        let err = decode_jpeg_thumbnail(b"", "decode_thumbnail", Path::new("x")).unwrap_err();
        assert_eq!(err.kind(), Kind::Corrupt);
    }
}
