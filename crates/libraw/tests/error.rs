use std::error::Error as _;
use std::path::Path;

use rsb_libraw::error::{
    ConvertSnafu, DimensionsSnafu, Error, FormatSnafu, InitSnafu, JpegSnafu, Kind, MakeImageSnafu,
    MakeThumbSnafu, NoPreviewSnafu, NoThumbnailSnafu, OpenSnafu, ProcessSnafu,
    ThumbnailFormatSnafu, UnpackSnafu, UnpackThumbSnafu,
};
use rsb_libraw::ffi::{self, ErrorCode};
use snafu::IntoError;

const OP: &str = "decode";

fn path() -> &'static Path {
    Path::new("/photos/DSCF0001.RAF")
}

fn call(code: ErrorCode) -> ffi::Error {
    ffi::Error::Call {
        call: "libraw_unpack",
        code,
        message: "LibRaw's text".into(),
        location: snafu::GenerateImplicitData::generate(),
    }
}

fn unpack(source: ffi::Error) -> Error {
    UnpackSnafu {
        op: OP,
        path: path(),
    }
    .into_error(source)
}

fn open(source: ffi::Error, exists: bool) -> Error {
    OpenSnafu {
        op: OP,
        path: path(),
        exists,
    }
    .into_error(source)
}

fn jpeg() -> jpeg_decoder::Error {
    jpeg_decoder::Error::Format("bad marker".into())
}

fn dimensions() -> Error {
    DimensionsSnafu { what: "10 pixels" }.build()
}

#[test]
fn each_libraw_code_gives_the_kind_it_always_gave() {
    let cases = [
        (ErrorCode::IoError, Kind::Io),
        (ErrorCode::InputClosed, Kind::Io),
        (ErrorCode::InsufficientMemory, Kind::Resource),
        (ErrorCode::MempoolOverflow, Kind::Resource),
        (ErrorCode::FileUnsupported, Kind::Unsupported),
        (ErrorCode::UnsupportedThumbnail, Kind::Unsupported),
        (ErrorCode::NotImplemented, Kind::Unsupported),
        (ErrorCode::NoThumbnail, Kind::Unsupported),
        (ErrorCode::RequestForNonexistentThumbnail, Kind::Unsupported),
        (ErrorCode::Unspecified, Kind::Corrupt),
        (ErrorCode::RequestForNonexistentImage, Kind::Corrupt),
        (ErrorCode::OutOfOrderCall, Kind::Corrupt),
        (ErrorCode::DataError, Kind::Corrupt),
        (ErrorCode::CancelledByCallback, Kind::Corrupt),
        (ErrorCode::BadCrop, Kind::Corrupt),
        (ErrorCode::TooBig, Kind::Corrupt),
        (ErrorCode::Other(999999), Kind::Corrupt),
    ];
    for (code, kind) in cases {
        assert_eq!(unpack(call(code)).kind(), kind, "{code:?}");
    }
}

#[test]
fn each_ffi_error_gives_the_kind_it_always_gave() {
    // ffi builds its own Open; a missing file is the way to get one.
    let open_failed = ffi::Processor::new()
        .unwrap()
        .open_file(Path::new("/does/not/exist/4c1d.raf"))
        .unwrap_err();
    assert!(matches!(open_failed, ffi::Error::Open { .. }));
    assert_eq!(open(open_failed, false).kind(), Kind::Io);
    assert_eq!(open(ffi::Error::PathNul, false).kind(), Kind::Io);
    let init = InitSnafu {
        op: OP,
        path: path(),
    }
    .into_error(ffi::Error::Init);
    assert_eq!(init.kind(), Kind::Resource);
}

#[test]
fn an_io_error_opening_a_file_that_exists_is_corrupt() {
    assert_eq!(open(call(ErrorCode::IoError), true).kind(), Kind::Corrupt);
    assert_eq!(open(call(ErrorCode::IoError), false).kind(), Kind::Io);
    assert_eq!(
        open(call(ErrorCode::FileUnsupported), true).kind(),
        Kind::Unsupported
    );
}

#[test]
fn checks_made_by_this_crate_give_their_kind() {
    let p = path();
    let jpeg = JpegSnafu { op: OP, path: p }.into_error(jpeg());
    assert_eq!(jpeg.kind(), Kind::Corrupt);
    assert_eq!(dimensions().kind(), Kind::Corrupt);
    let convert = ConvertSnafu { op: OP, path: p }.into_error(dimensions());
    assert_eq!(convert.kind(), Kind::Corrupt);
    let unsupported = [
        FormatSnafu {
            op: OP,
            path: p,
            what: "8-bit",
        }
        .build(),
        ThumbnailFormatSnafu {
            op: OP,
            path: p,
            what: "Bitmap",
        }
        .build(),
        NoThumbnailSnafu { op: OP, path: p }.build(),
        NoPreviewSnafu { op: OP, path: p }.build(),
    ];
    for e in unsupported {
        assert_eq!(e.kind(), Kind::Unsupported, "{e:?}");
    }
}

#[test]
fn every_variant_has_its_code() {
    let p = path();
    let cases = [
        (
            InitSnafu { op: OP, path: p }.into_error(ffi::Error::Init),
            "libraw.init.init",
        ),
        (open(ffi::Error::PathNul, false), "libraw.open.path_nul"),
        (
            open(call(ErrorCode::FileUnsupported), true),
            "libraw.open.call",
        ),
        (unpack(call(ErrorCode::DataError)), "libraw.unpack.call"),
        (
            UnpackThumbSnafu { op: OP, path: p }.into_error(call(ErrorCode::NoThumbnail)),
            "libraw.unpack_thumb.call",
        ),
        (
            ProcessSnafu { op: OP, path: p }.into_error(call(ErrorCode::DataError)),
            "libraw.process.call",
        ),
        (
            MakeImageSnafu { op: OP, path: p }.into_error(call(ErrorCode::OutOfOrderCall)),
            "libraw.make_image.call",
        ),
        (
            MakeThumbSnafu { op: OP, path: p }.into_error(call(ErrorCode::InsufficientMemory)),
            "libraw.make_thumb.call",
        ),
        (
            JpegSnafu { op: OP, path: p }.into_error(jpeg()),
            "libraw.jpeg",
        ),
        (
            ConvertSnafu { op: OP, path: p }.into_error(dimensions()),
            "libraw.convert.dimensions",
        ),
        (
            FormatSnafu {
                op: OP,
                path: p,
                what: "8-bit",
            }
            .build(),
            "libraw.format",
        ),
        (
            ThumbnailFormatSnafu {
                op: OP,
                path: p,
                what: "Bitmap",
            }
            .build(),
            "libraw.thumbnail_format",
        ),
        (dimensions(), "libraw.dimensions"),
        (
            NoThumbnailSnafu { op: OP, path: p }.build(),
            "libraw.no_thumbnail",
        ),
        (
            NoPreviewSnafu { op: OP, path: p }.build(),
            "libraw.no_preview",
        ),
    ];
    for (e, code) in cases {
        assert_eq!(e.code(), code, "{e:?}");
    }
}

#[test]
fn a_typed_match_reaches_the_libraw_code_and_source_returns_the_cause() {
    let e = unpack(call(ErrorCode::DataError));
    assert!(matches!(
        e,
        Error::Unpack {
            source: ffi::Error::Call {
                code: ErrorCode::DataError,
                ..
            },
            ..
        }
    ));
    let source = e.source().unwrap();
    assert!(source.downcast_ref::<ffi::Error>().is_some());

    let e = JpegSnafu {
        op: OP,
        path: path(),
    }
    .into_error(jpeg());
    let source = e.source().unwrap();
    assert!(source.downcast_ref::<jpeg_decoder::Error>().is_some());

    assert!(
        NoPreviewSnafu {
            op: OP,
            path: path()
        }
        .build()
        .source()
        .is_none()
    );
}

#[test]
fn error_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Error>();
}
