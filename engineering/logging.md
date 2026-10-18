# Logging

This repository follows `../../rules/standards/logging.md`. It is a
library: it logs at `trace` only and never a failure, which it returns.
`Processor` logs each LibRaw call with its name and duration at `trace`,
through one helper in `crates/ffi/src/processor.rs`. `rsb-libraw` logs
nothing.

**Check.** `cargo xtask errors` fails on `warn!` or `error!` anywhere, since
a library has no boundary module, and on `println!`, `eprintln!` or the
`log` crate outside `xtask`'s `main` and `crates/ffi/build.rs`.
