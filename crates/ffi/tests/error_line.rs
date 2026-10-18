//! The golden line for this crate's layer: `Open` over a missing file. By
//! default it carries one location; after `show_locations(false)` it is
//! pinned exactly. One test function, because `show_locations` is
//! process-wide.

use rsb_libraw_ffi::{Error, Processor};

#[test]
fn open_on_a_missing_file_has_one_location_and_one_line_without() {
    let path = std::env::temp_dir().join("rsb_libraw_ffi_does_not_exist_4c1d.raf");
    let mut processor = Processor::new().unwrap();
    let e = processor.open_file(&path).unwrap_err();
    assert!(matches!(e, Error::Open { .. }), "{e:?}");
    assert_eq!(e.code(), "libraw_ffi.open");

    let shown = e.to_string();
    assert_eq!(shown.matches(" (at ").count(), 1, "one location: {shown}");
    assert!(
        shown.starts_with("cannot open file (at crates/ffi/src/processor.rs:"),
        "the location follows the operation: {shown}"
    );

    rsb_errors::show_locations(false);
    assert_eq!(
        e.to_string(),
        "cannot open file: No such file or directory (os error 2)"
    );
}
