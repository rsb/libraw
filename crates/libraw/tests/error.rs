use std::error::Error as _;

use rsb_libraw::error::{Cause, Error, ErrorCode, LibRawError};

fn call(code: ErrorCode) -> LibRawError {
    LibRawError::Call {
        call: "libraw_test",
        code,
        message: "libraw text".to_string(),
    }
}

#[test]
fn io_failures_are_io() {
    let open = LibRawError::Open(std::io::Error::from(std::io::ErrorKind::NotFound));
    assert!(matches!(Error::from(open), Error::Io(_)));
    assert!(matches!(Error::from(LibRawError::PathNul), Error::Io(_)));
    for code in [ErrorCode::IoError, ErrorCode::InputClosed] {
        assert!(matches!(Error::from(call(code)), Error::Io(_)), "{code:?}");
    }
}

#[test]
fn memory_failures_are_resource() {
    assert!(matches!(Error::from(LibRawError::Init), Error::Resource(_)));
    for code in [ErrorCode::InsufficientMemory, ErrorCode::MempoolOverflow] {
        assert!(
            matches!(Error::from(call(code)), Error::Resource(_)),
            "{code:?}"
        );
    }
}

#[test]
fn unhandled_formats_are_unsupported() {
    for code in [
        ErrorCode::FileUnsupported,
        ErrorCode::UnsupportedThumbnail,
        ErrorCode::NotImplemented,
        ErrorCode::NoThumbnail,
        ErrorCode::RequestForNonexistentThumbnail,
    ] {
        assert!(
            matches!(
                Error::from(call(code)),
                Error::Unsupported(Cause::LibRaw(_))
            ),
            "{code:?}"
        );
    }
}

#[test]
fn other_codes_are_corrupt() {
    for code in [
        ErrorCode::Unspecified,
        ErrorCode::RequestForNonexistentImage,
        ErrorCode::OutOfOrderCall,
        ErrorCode::DataError,
        ErrorCode::CancelledByCallback,
        ErrorCode::BadCrop,
        ErrorCode::TooBig,
        ErrorCode::Other(999999),
    ] {
        assert!(
            matches!(Error::from(call(code)), Error::Corrupt(Cause::LibRaw(_))),
            "{code:?}"
        );
    }
}

/// Each level's `Display`, joined the way a caller walking `source()` would.
fn chain(err: &Error) -> Vec<String> {
    let mut texts = vec![err.to_string()];
    let mut source = err.source();
    while let Some(s) = source {
        texts.push(s.to_string());
        source = s.source();
    }
    texts
}

#[test]
fn the_chain_reaches_libraw_once() {
    for err in [
        Error::from(call(ErrorCode::IoError)),
        Error::from(call(ErrorCode::InsufficientMemory)),
        Error::from(call(ErrorCode::FileUnsupported)),
        Error::from(call(ErrorCode::DataError)),
    ] {
        let texts = chain(&err);
        assert_eq!(texts.len(), 2, "{texts:?}");
        assert!(!texts[0].contains("libraw text"), "{texts:?}");
        assert_eq!(texts[1], "libraw_test failed: libraw text");
    }
}

#[test]
fn the_chain_reaches_a_check() {
    let err = Error::Corrupt(Cause::Check("width * height overflows usize".into()));
    assert_eq!(
        chain(&err),
        ["corrupt data", "width * height overflows usize"]
    );
}
