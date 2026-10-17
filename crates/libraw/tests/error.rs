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
    assert!(matches!(Error::from(open), Error::Io { .. }));
    assert!(matches!(
        Error::from(LibRawError::PathNul),
        Error::Io { .. }
    ));
    for code in [ErrorCode::IoError, ErrorCode::InputClosed] {
        assert!(
            matches!(Error::from(call(code)), Error::Io { .. }),
            "{code:?}"
        );
    }
}

#[test]
fn memory_failures_are_resource() {
    assert!(matches!(
        Error::from(LibRawError::Init),
        Error::Resource { .. }
    ));
    for code in [ErrorCode::InsufficientMemory, ErrorCode::MempoolOverflow] {
        assert!(
            matches!(Error::from(call(code)), Error::Resource { .. }),
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
                Error::Unsupported {
                    cause: Cause::LibRaw(_),
                    ..
                }
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
            matches!(
                Error::from(call(code)),
                Error::Corrupt {
                    cause: Cause::LibRaw(_),
                    ..
                }
            ),
            "{code:?}"
        );
    }
}

#[test]
fn the_message_reads_operation_cause_category() {
    let err = Error::from(call(ErrorCode::DataError)).during("LibRawAdapter::decode");
    assert_eq!(
        err.to_string(),
        "LibRawAdapter::decode: libraw_test failed: libraw text: corrupt data"
    );
}

#[test]
fn each_category_ends_the_message() {
    for (code, category) in [
        (ErrorCode::IoError, "i/o failure"),
        (ErrorCode::InsufficientMemory, "resource exhaustion"),
        (ErrorCode::FileUnsupported, "unsupported format"),
        (ErrorCode::DataError, "corrupt data"),
    ] {
        assert_eq!(
            Error::from(call(code)).to_string(),
            format!("libraw_test failed: libraw text: {category}")
        );
    }
}

#[test]
fn a_check_error_without_an_operation_is_message_then_category() {
    let err = Error::corrupt("width * height overflows usize");
    assert_eq!(
        err.to_string(),
        "width * height overflows usize: corrupt data"
    );
}

/// A reporter that walks `source()` must not print the cause twice.
#[test]
fn the_message_is_not_repeated_by_source() {
    assert!(Error::from(call(ErrorCode::DataError)).source().is_none());
}

#[test]
fn the_location_is_where_the_error_was_raised() {
    let line = line!() + 1;
    let err = Error::from(call(ErrorCode::DataError));
    assert_eq!(err.location().file(), file!());
    assert_eq!(err.location().line(), line);
}
