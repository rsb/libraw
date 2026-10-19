//! The lines the owner approved for each public method on a missing, an
//! unreadable and a non-RAW file. By default each wrapping layer carries one
//! location; after `show_locations(false)` each line is pinned exactly. One
//! test function, because `show_locations` is process-wide.

use std::path::{Path, PathBuf};

use rsb_libraw::{Error, ImageBuffer, LibRawAdapter, RawDecode};

/// A directory of the three failing files, removed on drop.
struct Fixtures(PathBuf);

impl Fixtures {
    fn new() -> std::io::Result<Self> {
        let dir = std::env::temp_dir().join(format!("rsb_libraw_lines_{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("unreadable.RAF"), vec![0u8; 4096])?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::Permissions::from_mode(0o000);
            std::fs::set_permissions(dir.join("unreadable.RAF"), mode)?;
        }
        std::fs::write(dir.join("notes.txt"), "Shopping list\n".repeat(400))?;
        Ok(Self(dir))
    }
}

impl Drop for Fixtures {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

type Method = fn(&LibRawAdapter, &Path) -> Result<ImageBuffer, Error>;

const METHODS: [(&str, Method); 3] = [
    ("decode", LibRawAdapter::decode),
    ("decode_preview", LibRawAdapter::decode_preview),
    ("decode_thumbnail", LibRawAdapter::decode_thumbnail),
];

/// Each case: file name, locations by default, the line without them.
const CASES: [(&str, usize, &str); 3] = [
    (
        "DSCF0001.RAF",
        2,
        "open: cannot open file: No such file or directory (os error 2)",
    ),
    (
        "unreadable.RAF",
        2,
        "open: cannot open file: Permission denied (os error 13)",
    ),
    (
        "notes.txt",
        2,
        "open: libraw_open_file failed: Unsupported file format or not RAW file",
    ),
];

#[test]
fn each_method_prints_the_approved_line_for_each_failing_file() {
    let fixtures = Fixtures::new().unwrap();
    // Root reads any file, so the unreadable case cannot fail as it should.
    let unreadable_fails = std::fs::File::open(fixtures.0.join("unreadable.RAF")).is_err();
    let cases: Vec<_> = CASES
        .iter()
        .filter(|(name, ..)| unreadable_fails || *name != "unreadable.RAF")
        .collect();

    let mut errors = Vec::new();
    for (op, method) in METHODS {
        for (name, locations, line) in &cases {
            let path = fixtures.0.join(name);
            let e = method(&LibRawAdapter, &path).unwrap_err();
            let shown = e.to_string();
            assert_eq!(
                shown.matches(" (at ").count(),
                *locations,
                "one location per wrapping layer: {shown}"
            );
            assert!(
                shown.starts_with(&format!(
                    "{op} {}: open (at crates/libraw/src/adapter.rs:",
                    path.display()
                )),
                "the location follows the step: {shown}"
            );
            errors.push((format!("{op} {}: {line}", path.display()), e));
        }
    }

    rsb_errors::show_locations(false);
    for (line, e) in errors {
        assert_eq!(e.to_string(), line);
    }
}
