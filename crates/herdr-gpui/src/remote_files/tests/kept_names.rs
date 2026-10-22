//! The readable name a file copy that may hold edits is kept under for good
//! beside the folder that took its place.

use super::*;
use crate::remote_files::{
    cache::kept_name,
    folders::{keep_beside, renames_after},
};
use std::io::ErrorKind;

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

#[test]
fn a_multi_byte_name_is_cut_on_a_character_boundary() {
    let name = format!("{}.md", "é".repeat(126));
    assert_eq!(name.len(), 255);
    let kept = kept_name(&name, 1);
    assert!(kept.len() <= 255, "{} bytes", kept.len());
    let stem = kept.strip_suffix(".md.kept").expect("the extension stays");
    assert_eq!(stem, "é".repeat(123));
}

#[test]
fn a_name_with_no_extension_is_cut_whole() {
    assert_eq!(kept_name("Makefile", 1), "Makefile.kept");
    assert_eq!(kept_name("Makefile", 2), "Makefile.2.kept");
    let long = "a".repeat(300);
    assert_eq!(kept_name(&long, 1), format!("{}.kept", "a".repeat(250)));
}

#[test]
fn a_leading_dot_is_no_extension() {
    assert_eq!(kept_name(".bashrc", 1), ".bashrc.kept");
    let long = format!(".{}", "b".repeat(300));
    assert_eq!(kept_name(&long, 1), format!(".{}.kept", "b".repeat(249)));
}

#[test]
fn a_name_ending_in_a_dot_keeps_it() {
    assert_eq!(kept_name("notes.", 1), "notes..kept");
    let long = format!("{}.", "a".repeat(300));
    assert_eq!(kept_name(&long, 1), format!("{}..kept", "a".repeat(249)));
}

#[test]
fn an_extension_too_long_to_keep_is_cut_with_the_name() {
    let name = format!("a.{}", "x".repeat(260));
    let kept = kept_name(&name, 1);
    assert_eq!(kept, format!("a.{}.kept", "x".repeat(248)));
    assert_eq!(kept.len(), 255);
}

#[test]
fn a_name_shaped_like_a_transient_one_starts_at_two() {
    assert_eq!(kept_name(".17", 1), ".17.2.kept");
    assert_eq!(kept_name(".17", 2), ".17.3.kept");
    // Only a dot and digits alone take that shape.
    assert_eq!(kept_name(".17a", 1), ".17a.kept");
    assert_eq!(kept_name("17", 1), "17.kept");
}

#[test]
fn a_hard_link_refused_for_any_other_reason_falls_back_to_a_rename() {
    for kind in [
        ErrorKind::Unsupported,
        ErrorKind::PermissionDenied,
        ErrorKind::Other,
        ErrorKind::InvalidInput,
    ] {
        assert!(renames_after(kind), "{kind:?}");
    }
    assert!(
        !renames_after(ErrorKind::AlreadyExists),
        "the name is taken"
    );
    assert!(!renames_after(ErrorKind::NotFound), "the copy is gone");
}
