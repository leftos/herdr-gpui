//! The readable name a file copy that may hold edits is kept under for good
//! beside the folder that took its place.

use super::*;
use crate::remote_files::folders::keep_beside;

/// A copy a folder transfer moved aside under a transient name in `dir`.
fn moved_aside(dir: &Path, serial: u32, bytes: &[u8]) -> PathBuf {
    let moved = dir.join(format!(".{serial}.kept"));
    std::fs::write(&moved, bytes).unwrap();
    moved
}

fn name_of(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

#[test]
fn a_kept_copy_of_a_250_byte_name_fits_a_file_name() {
    let dir = tempfile::tempdir().unwrap();
    let name = format!("{}.md", "n".repeat(247));
    assert_eq!(name.len(), 250);
    let local = dir.path().join(&name);
    let first = keep_beside(&moved_aside(dir.path(), 0, b"one"), &local);
    assert_eq!(name_of(&first), format!("{name}.kept"));
    let second = keep_beside(&moved_aside(dir.path(), 1, b"two"), &local);
    let second_name = name_of(&second);
    assert!(second_name.len() <= 255, "{} bytes", second_name.len());
    assert!(second_name.ends_with(".md.2.kept"), "{second_name}");
    assert!(second_name.starts_with("nnnn"), "{second_name}");
    assert_eq!(std::fs::read(&first).unwrap(), b"one");
    assert_eq!(std::fs::read(&second).unwrap(), b"two");
}

#[test]
fn a_second_keep_beside_the_same_folder_gets_the_next_number() {
    let dir = tempfile::tempdir().unwrap();
    let local = dir.path().join("notes.md");
    std::fs::create_dir(&local).unwrap();
    let first_moved = moved_aside(dir.path(), 0, b"one");
    let first = keep_beside(&first_moved, &local);
    let second_moved = moved_aside(dir.path(), 1, b"two");
    let second = keep_beside(&second_moved, &local);
    assert_eq!(first, dir.path().join("notes.md.kept"));
    assert_eq!(second, dir.path().join("notes.md.2.kept"));
    assert_eq!(
        std::fs::read(&first).unwrap(),
        b"one",
        "the first is not replaced"
    );
    assert_eq!(std::fs::read(&second).unwrap(), b"two");
    assert!(
        !first_moved.exists() && !second_moved.exists(),
        "no transient name is left"
    );
}
