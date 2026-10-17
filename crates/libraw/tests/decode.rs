use std::path::{Path, PathBuf};

use rsb_libraw::Error;
use rsb_libraw::error::Cause;
use rsb_libraw::{LibRawAdapter, RawDecode};

fn missing_path() -> PathBuf {
    std::env::temp_dir().join("does_not_exist_9f8a7b.cr2")
}

/// A file holding `bytes`, removed on drop. The name carries the process id
/// so concurrent test runs do not share it.
struct TempFile(PathBuf);

impl TempFile {
    fn new(name: &str, bytes: &[u8]) -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!("rsb_libraw_{}_{name}", std::process::id()));
        std::fs::write(&path, bytes)?;
        Ok(Self(path))
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).ok();
    }
}

#[test]
fn decode_nonexistent_file_returns_io() {
    let adapter = LibRawAdapter;
    let result = adapter.decode(&missing_path());
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::Io { .. }));
}

#[test]
fn decode_thumbnail_nonexistent_file_returns_io() {
    let adapter = LibRawAdapter;
    let result = adapter.decode_thumbnail(&missing_path());
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::Io { .. }));
}

#[test]
fn decode_preview_nonexistent_file_returns_io() {
    let adapter = LibRawAdapter;
    let result = adapter.decode_preview(&missing_path());
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::Io { .. }));
}

#[test]
fn decode_non_raw_file_returns_corrupt() {
    let file = TempFile::new("garbage.cr2", b"this is not a raw file at all").unwrap();
    let err = LibRawAdapter.decode(&file.0).unwrap_err();
    assert!(matches!(err, Error::Corrupt { .. }));
}

#[test]
fn decode_thumbnail_non_raw_file_returns_corrupt() {
    let file = TempFile::new("garbage.nef", b"not a nef file").unwrap();
    let err = LibRawAdapter.decode_thumbnail(&file.0).unwrap_err();
    assert!(matches!(err, Error::Corrupt { .. }));
}

#[test]
fn decode_preview_non_raw_file_returns_corrupt() {
    let file = TempFile::new("garbage.arw", b"not an arw file").unwrap();
    let err = LibRawAdapter.decode_preview(&file.0).unwrap_err();
    assert!(matches!(err, Error::Corrupt { .. }));
}

#[test]
#[cfg(unix)]
fn decode_unreadable_file_returns_io() {
    use std::os::unix::fs::PermissionsExt;
    let file = TempFile::new("unreadable.cr2", b"raw bytes").unwrap();
    std::fs::set_permissions(&file.0, std::fs::Permissions::from_mode(0o000)).unwrap();
    // Root reads any file, so there is nothing to test.
    if std::fs::File::open(&file.0).is_ok() {
        return;
    }
    let err = LibRawAdapter.decode(&file.0).unwrap_err();
    assert!(matches!(err, Error::Io { .. }));
}

#[test]
fn decode_path_with_interior_null_returns_io() {
    let adapter = LibRawAdapter;
    let result = adapter.decode(&std::env::temp_dir().join("bad\0path.cr2"));

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, Error::Io { .. }));
}

#[test]
fn decode_thumbnail_path_with_interior_null_returns_io() {
    let adapter = LibRawAdapter;
    let result = adapter.decode_thumbnail(&std::env::temp_dir().join("bad\0path.cr2"));

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, Error::Io { .. }));
}

#[test]
fn decode_preview_path_with_interior_null_returns_io() {
    let adapter = LibRawAdapter;
    let result = adapter.decode_preview(&std::env::temp_dir().join("bad\0path.cr2"));

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, Error::Io { .. }));
}

struct DecodeOnly;

impl RawDecode for DecodeOnly {
    fn decode(&self, _path: &Path) -> Result<rsb_libraw::ImageBuffer, Error> {
        Err(Error::unsupported("stub"))
    }
}

#[test]
fn default_decode_thumbnail_returns_unsupported() {
    let d = DecodeOnly;
    let result = d.decode_thumbnail(&missing_path());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(&err, Error::Unsupported { cause: Cause::Check(m), .. } if m.contains("thumbnail"))
    );
}

#[test]
fn default_decode_preview_returns_unsupported() {
    let d = DecodeOnly;
    let result = d.decode_preview(&missing_path());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(&err, Error::Unsupported { cause: Cause::Check(m), .. } if m.contains("preview"))
    );
}

#[test]
fn the_message_names_the_operation_the_cause_and_the_category() {
    let file = TempFile::new("garbage.raf", b"not a raf file").unwrap();
    let err = LibRawAdapter.decode_thumbnail(&file.0).unwrap_err();
    let text = err.to_string();
    assert!(
        text.starts_with("LibRawAdapter::decode_thumbnail: libraw_open_file failed: "),
        "{text}"
    );
    assert!(text.ends_with(": corrupt data"), "{text}");
}

#[test]
fn a_missing_file_says_why_it_could_not_be_opened() {
    let err = LibRawAdapter.decode(&missing_path()).unwrap_err();
    let text = err.to_string();
    assert!(
        text.starts_with("LibRawAdapter::decode: cannot open file: "),
        "{text}"
    );
    assert!(text.ends_with(": i/o failure"), "{text}");
    assert!(
        err.location().file().ends_with("adapter.rs"),
        "{}",
        err.location()
    );
}
