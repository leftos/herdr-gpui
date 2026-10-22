//! The PowerShell probe's size is the sum of the regular files' bytes, the
//! same total the POSIX probe gives for the same tree.

use super::*;

#[test]
fn powershell_probe_sums_the_regular_files_bytes() {
    let shells = shells();
    if shells.is_empty() {
        eprintln!("skipped: no PowerShell on this machine");
        return;
    }
    for shell in shells {
        eprintln!("running under {shell}");
        let fixture = Fixture {
            shell,
            home: tempfile::tempdir().unwrap(),
        };
        small_tree(&fixture.path("tree"));
        let probed = fixture.probe(&fixture.text("tree"));
        assert_code(&probed, 0);
        let probed = parse_probe_output(&probed.stdout).unwrap();
        assert_eq!(probed.bytes, SMALL_TREE_BYTES, "{shell}");
        assert_eq!(probed.files, 3, "{shell}");
    }
}
