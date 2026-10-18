# Panics

This repository follows `../../rules/standards/panics.md`. It is a library
with no dispatcher, so it has no `catch_unwind`, and it hands LibRaw no
`extern "C"` callback; the first one added begins with `catch_unwind`.
`unsafe` lives only in `rsb-libraw-ffi`, where each block starts with
`// SAFETY:`. `crates/ffi/build.rs` may `unwrap` under a file-level allow:
a build script that cannot proceed should stop the build. The bindgen
output in `crates/ffi/src/lib.rs` allows `undocumented_unsafe_blocks`,
because it is generated.

**Check.** `cargo clippy --workspace --all-targets -- -D warnings` through
the standard's table in `[workspace.lints]` and `clippy.toml`.
`rsb-libraw` takes the workspace lints, which forbid `unsafe_code`;
`rsb-libraw-ffi` repeats the table without `unsafe_code` and adds
`undocumented_unsafe_blocks`. `cargo xtask errors` checks the release
profile, bare panics and `catch_unwind`.
