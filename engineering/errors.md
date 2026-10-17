# Errors

## errors

Each crate has one error enum: `rsb-libraw-ffi`'s `Error`, derived with
`thiserror`, and `rsb-libraw`'s `Error`. Every variant of `rsb-libraw`'s
`Error` is a category the caller acts on: `Io`, `Unsupported`, `Corrupt`,
`Resource`. Its `Display` is one line a person can read without walking a
chain: the operation, the cause, then the category, as in
`LibRawAdapter::decode: libraw_unpack failed: <libraw text>: corrupt data`.
Its `source()` is `None`, so a reporter that walks the chain does not
print the cause twice; code that needs the typed cause matches the
variant's `cause` field. Each error records where it was raised:
`#[track_caller]` on the constructors and on `From<LibRawError>`, so `?`
records its own line. An ffi error enters `rsb-libraw` through `From`, so
`?` carries it. Results are not dropped: `unused_must_use` and
`let_underscore_must_use` are deny. `anyhow`, `eyre`, other error crates
and `Box<dyn Error>` do not appear.

**Why.** The owner reads these errors to find what failed. One line that
names the operation, the cause and the category answers that without
extra steps, and the typed variant still lets code decide what to do.

**Paths.** Both crates.

**Check.** The `errors` step in `.github/workflows/check.yml` fails on a
`Cargo.toml` that lists `anyhow`, `eyre`, `color-eyre`, `failure`,
`error-chain` or `snafu`, and on a `.rs` line that boxes a `dyn Error`.
`cargo clippy` enforces the must-use lints. `crates/libraw/tests/error.rs`
pins the line's shape, the `None` source and the location.

**Example.** `Error`, `Cause` and `From<LibRawError>` in
`crates/libraw/src/error.rs`.
