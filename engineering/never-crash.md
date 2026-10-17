# Never crash

## never-crash

Outside tests and `crates/ffi/build.rs`, no code can panic: no `unwrap`,
`expect`, `panic!` or indexing. Every failure reaches the caller as an
`Error`. `unsafe` is allowed only in `rsb-libraw-ffi`, and each `unsafe`
block there starts with a `// SAFETY:` comment saying why it is sound.

**Why.** Lumen calls this crate from an editor, and an editor that crashes
loses work. Unsafe code belongs only where LibRaw is called through FFI,
in one crate a reviewer can read whole.

**Paths.** Both crates; `crates/ffi` for `unsafe`. `crates/ffi/build.rs`
may `unwrap` and `expect`, under a file-level allow: a build script that
cannot proceed should stop the build. The bindgen output in
`crates/ffi/src/lib.rs` allows `indexing_slicing`, because its layout
checks run at compile time, and `undocumented_unsafe_blocks`, because it is
generated.

**Check.** `cargo clippy --workspace --all-targets -- -D warnings`, through
`[workspace.lints]` in the root `Cargo.toml` and `clippy.toml`, which allows
the four in tests. `rsb-libraw` takes the workspace lints, which forbid
`unsafe_code`. `rsb-libraw-ffi` repeats the table without `unsafe_code` and
adds `undocumented_unsafe_blocks`.

**Contract.** `RawDecode` in `crates/libraw/src/decode.rs`: every method
returns `Result<ImageBuffer, Error>`. `Processor` in
`crates/ffi/src/processor.rs`: every LibRaw call returns `Result<_, Error>`.
