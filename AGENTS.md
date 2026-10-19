# AGENTS.md

Read `README.md` first. Sessions run from this repository. Standards shared by
the rsb apps live in `../rules`; read its `README.md`.

Commits follow Conventional Commits. Never add `Co-Authored-By` or any other AI
attribution to commits or pull requests. Outside ticket work, do not commit or
push unless asked. Dates and times are America/Los_Angeles.

Checks. Add `--message-format short` to check, test and clippy. CI runs
all but the typecheck and the single test file on every push.

- Typecheck: `cargo check --workspace --all-targets`
- One test file: `cargo test -p rsb-libraw --test <file>`
- Full suite: `cargo test --workspace`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Format: `cargo fmt --check`
- Errors, panics and logging: `cargo xtask errors`

The rules these enforce are in `engineering/`: `errors.md`, `panics.md` and
`logging.md`, each a pointer at the shared standard in `../rules`.
