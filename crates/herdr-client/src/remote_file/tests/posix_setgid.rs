//! Pushes into folders whose set-group-ID bit the push's work folder
//! inherits, run under this machine's `/bin/sh`.

use super::*;

/// Linux gives a folder made inside a set-group-ID folder the bit too, so the
/// private work folder lists as `drwx--S---`; a group-writable shared project
/// folder is the common case.
#[cfg(target_os = "linux")]
#[test]
fn push_succeeds_in_a_setgid_folder_the_user_owns() {
    let fixture = Fixture::new();
    std::fs::write(fixture.path("file"), b"old").unwrap();
    std::fs::set_permissions(fixture.home.path(), std::fs::Permissions::from_mode(0o2775)).unwrap();
    let pushed = fixture.push(&fixture.text("file"), Expect::Any, b"new");
    assert!(pushed.is_ok(), "{pushed:?}");
    assert_eq!(std::fs::read(fixture.path("file")).unwrap(), b"new");
    assert_eq!(entries(fixture.home.path()), ["file"]);
}

/// The `mktemp` stand-in makes the work folder `drwx--s---`: set-group-ID,
/// and its group may enter it.
#[test]
fn push_refuses_a_work_folder_its_group_can_enter() {
    let mut fixture = Fixture::new();
    std::fs::write(fixture.path("file"), b"old").unwrap();
    let body = format!(
        "case \"$1\" in -d)\n  made=$('{mktemp}' \"$@\") || exit 1\n  '{chmod}' 2710 \"$made\" || exit 1\n  \
         printf '%s\\n' \"$made\"\n  exit 0 ;;\nesac\nexec '{mktemp}' \"$@\"\n",
        mktemp = real_tool("mktemp").display(),
        chmod = real_tool("chmod").display(),
    );
    let _bin = fixture.shim("mktemp", &body);
    let pushed = fixture.push(&fixture.text("file"), Expect::Any, b"new");
    assert!(matches!(pushed, Err(Error::SharedFolder)), "{pushed:?}");
    assert_eq!(std::fs::read(fixture.path("file")).unwrap(), b"old");
    assert_eq!(entries(fixture.home.path()), ["file"]);
}
