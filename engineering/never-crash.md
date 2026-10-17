# Never crash

## never-crash

Outside tests and `crates/ffi/build.rs`, no code can panic: no `unwrap`,
`expect`, `panic!` or indexing. Every failure reaches the caller as an
`Error`. `unsafe` is allowed in `rsb-libraw-ffi` and in
`crates/libraw/src/adapter.rs` only.

**Why.** Lumen calls this crate from an editor, and an editor that crashes
loses work. Unsafe code belongs only where LibRaw is called through FFI.

**Paths.** Both crates. `crates/ffi` and `crates/libraw/src/adapter.rs` for
`unsafe`. `crates/ffi/build.rs` may `unwrap` and `expect`, under a file-level
allow: a build script that cannot proceed should stop the build. The bindgen
output in `crates/ffi/src/lib.rs` allows `indexing_slicing`, because its
layout checks run at compile time.

**Check.** `cargo clippy --workspace --all-targets -- -D warnings`, through
`[workspace.lints]` in the root `Cargo.toml` and `clippy.toml`, which allows
the four in tests. Both crates repeat the lint table without `unsafe_code`,
so no lint keeps `unsafe` inside `adapter.rs`; review does, until the unsafe
boundary moves into `rsb-libraw-ffi` and `rsb-libraw` forbids it.

**Contract.** `RawDecode` in `crates/libraw/src/decode.rs`: every method
returns `Result<ImageBuffer, Error>`.
