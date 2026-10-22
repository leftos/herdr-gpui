//! The POSIX probe's size is the sum of the regular files' bytes, as the
//! PowerShell probe's is, rather than the disk blocks they take.

use super::super::folder as scripts;
use super::*;

fn probe(fixture: &Fixture, name: &str) -> std::result::Result<ProbedFolder, Error> {
    let script = scripts::posix_probe_script(&fixture.text(name));
    probe_with(fixture.command(&script), &AtomicBool::new(false))
}

#[test]
fn posix_probe_sums_the_regular_files_bytes() {
    let fixture = Fixture::new();
    small_tree(&fixture.path("tree"));
    std::fs::create_dir(fixture.path("empty")).unwrap();

    let probed = probe(&fixture, "tree").unwrap();
    assert_eq!(
        probed.bytes, SMALL_TREE_BYTES,
        "the files' own bytes, not their disk blocks; the link adds nothing"
    );
    assert_eq!(probed.files, 3);

    let empty = probe(&fixture, "empty").unwrap();
    assert_eq!((empty.bytes, empty.files), (0, 0));
}

/// A host whose `stat` takes neither GNU nor BSD options still sums the
/// files' bytes, from `ls -lnd`.
#[test]
fn posix_probe_sums_the_bytes_without_a_usable_stat() {
    let mut fixture = Fixture::new();
    small_tree(&fixture.path("tree"));
    let _bin = fixture.shim("stat", "exit 1\n");

    let probed = probe(&fixture, "tree").unwrap();
    assert_eq!(probed.bytes, SMALL_TREE_BYTES);
    assert_eq!(probed.files, 3);
}
