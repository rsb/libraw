//! Workspace checks: `cargo xtask errors`.

use std::path::Path;
use std::process::ExitCode;

use rsb_errors::check::{Config, run};

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("errors") => errors(),
        _ => {
            eprintln!("usage: cargo xtask errors");
            ExitCode::FAILURE
        }
    }
}

/// The checks of the errors, panics and logging standards over the
/// workspace. A library has no dispatcher and no boundary module, so both
/// name a path that does not exist: a `catch_unwind`, `warn!` or `error!`
/// anywhere is a finding.
fn errors() -> ExitCode {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."));
    let config = Config {
        dispatcher: "src/none.rs".into(),
        boundary: "src/none.rs".into(),
        // A build script must println! its cargo directives.
        ignore: vec!["crates/ffi/build.rs".into()],
    };
    let findings = run(root, &config);
    for finding in &findings {
        eprintln!("{finding}");
    }
    if findings.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
