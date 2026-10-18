# Errors

This repository follows `../../rules/standards/errors.md`. Each crate has
one `snafu` enum in `src/error.rs`: `rsb_libraw_ffi::Error` for the C
boundary, `rsb_libraw::Error` for what a caller sees. Every variant of
`rsb_libraw::Error` carries the public method (`op`) and the path, and
`Error::kind()` derives the four-way `Kind` callers branch on from the
variant and LibRaw's code.

**Check.** `cargo xtask errors` runs `rsb_errors::check` over the
workspace, ignoring `crates/ffi/build.rs`, whose cargo directives must be
`println!`. `crates/libraw/tests/error_line.rs` pins the approved line for
each public method on a missing, an unreadable and a non-RAW file, and
`crates/ffi/tests/error_line.rs` the ffi layer. `cargo clippy` enforces
the must-use lints.
