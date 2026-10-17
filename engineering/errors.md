# Errors

## errors

Each crate has one error enum, derived with `thiserror`: `rsb-libraw-ffi`'s
`Error` and `rsb-libraw`'s `Error`. Every variant of `rsb-libraw`'s `Error`
is a category the caller acts on: `Io`, `Unsupported`, `Corrupt`,
`Resource`. A variant's `Display` states its own level; the detail is its
`source()`, so a caller walking the chain sees each fact once. An ffi error
enters `rsb-libraw` through `From`, so `?` carries it. Results are not
dropped: `unused_must_use` and `let_underscore_must_use` are deny.
`anyhow`, `eyre`, other error crates and `Box<dyn Error>` do not appear.

**Why.** A typed error says what went wrong and who can fix it, and the
chain reaches the leaf without wrap machinery. Lumen, the consumer, follows
the same rule and renders the chain itself.

**Paths.** Both crates.

**Check.** The `errors` step in `.github/workflows/check.yml` fails on a
`Cargo.toml` that lists `anyhow`, `eyre`, `color-eyre`, `failure`,
`error-chain` or `snafu`, and on a `.rs` line that boxes a `dyn Error`.
`cargo clippy` enforces the must-use lints.

**Example.** `Error`, `Cause` and `From<LibRawError>` in
`crates/libraw/src/error.rs`.
