use super::*;

fn link(path: &str, cwd: Option<&str>) -> FileLink {
    FileLink {
        path: path.into(),
        line: None,
        query: None,
        pane_id: "pane".into(),
        cwd: cwd.map(Into::into),
        explicit: false,
        remote: Some("me@devbox".into()),
        unjoined: None,
    }
}

/// The shell and path the host is asked for, or `None` when there is none.
fn asked(path: &str, cwd: Option<&str>) -> Option<(RemoteShell, String)> {
    RemotePath::from_link(&link(path, cwd)).map(|remote| (remote.shell, remote.path))
}

fn powershell(path: &str) -> Option<(RemoteShell, String)> {
    Some((RemoteShell::PowerShell, path.into()))
}

fn posix(path: &str) -> Option<(RemoteShell, String)> {
    Some((RemoteShell::Posix, path.into()))
}

#[test]
fn a_query_printed_after_the_path_is_not_the_host_s() {
    let page = link("/srv/www/index.html", None);
    let with_query = FileLink {
        query: Some("?a=1".into()),
        ..page.clone()
    };
    assert_eq!(
        RemotePath::from_link(&with_query),
        RemotePath::from_link(&page)
    );
    assert_eq!(
        asked("/srv/www/index.html", None),
        posix("/srv/www/index.html")
    );
}

#[test]
fn a_local_pane_has_no_remote_path() {
    let local = FileLink {
        remote: None,
        ..link("/home/me/a.md", None)
    };
    assert_eq!(RemotePath::from_link(&local), None);
}

#[test]
fn a_drive_path_is_read_in_powershell_with_backslashes() {
    assert_eq!(
        asked(r"C:\dev\notes.md", None),
        powershell(r"C:\dev\notes.md")
    );
    assert_eq!(
        asked("c:/dev/notes.md", None),
        powershell(r"c:\dev\notes.md")
    );
    assert_eq!(
        asked(r"D:/dev\sub/x.rs", Some("/home/me")),
        powershell(r"D:\dev\sub\x.rs")
    );
}

#[test]
fn a_drive_path_keeps_its_target_and_line() {
    let remote = RemotePath::from_link(&FileLink {
        line: Some(42),
        ..link(r"C:\dev\notes.md", None)
    })
    .unwrap();
    assert_eq!(
        remote,
        RemotePath {
            target: "me@devbox".into(),
            shell: RemoteShell::PowerShell,
            path: r"C:\dev\notes.md".into(),
            line: Some(42),
        }
    );
}

#[test]
fn a_file_url_drive_path_loses_its_leading_slash() {
    assert_eq!(
        asked("/C:/Users/me/a.md", None),
        powershell(r"C:\Users\me\a.md")
    );
    // A relative path under such a cwd joins it as a drive path too.
    assert_eq!(
        asked("a.md", Some("/C:/Users/me")),
        powershell(r"C:\Users\me\a.md")
    );
}

#[test]
fn a_unc_path_is_read_in_powershell() {
    assert_eq!(
        asked(r"\\server\share\a.md", None),
        powershell(r"\\server\share\a.md")
    );
    assert_eq!(
        asked("a.md", Some(r"\\server\share")),
        powershell(r"\\server\share\a.md")
    );
}

#[test]
fn a_posix_absolute_path_is_kept_as_is() {
    assert_eq!(asked("/home/me/x.md", None), posix("/home/me/x.md"));
    assert_eq!(
        asked("/srv/a b/../x.md", Some(r"C:\dev")),
        posix("/srv/a b/../x.md")
    );
}

#[test]
fn a_home_path_is_left_for_the_host_to_expand() {
    assert_eq!(asked("~/x.md", Some("/home/me")), posix("~/x.md"));
    assert_eq!(asked("~/x.md", None), posix("~/x.md"));
    assert_eq!(asked("~", None), posix("~"));
    // A Windows host reads it in PowerShell, by the cwd or the separator.
    assert_eq!(asked("~/x.md", Some(r"C:\Users\me")), powershell(r"~\x.md"));
    assert_eq!(asked(r"~\x.md", None), powershell(r"~\x.md"));
    // Another account's home is not resolved.
    assert_eq!(asked("~other/x.md", Some("/home/me")), None);
}

#[test]
fn a_relative_path_joins_a_posix_cwd_with_slashes() {
    assert_eq!(
        asked("src/main.rs", Some("/home/me/app")),
        posix("/home/me/app/src/main.rs")
    );
    assert_eq!(
        asked("./src/main.rs", Some("/home/me/app/")),
        posix("/home/me/app/src/main.rs")
    );
    assert_eq!(
        asked("../x.md", Some("/home/me/app")),
        posix("/home/me/app/../x.md")
    );
    assert_eq!(asked("a.md", Some("/")), posix("/a.md"));
}

#[test]
fn a_relative_path_joins_a_windows_cwd_with_backslashes() {
    assert_eq!(
        asked(r"src\main.rs", Some(r"C:\dev\app")),
        powershell(r"C:\dev\app\src\main.rs")
    );
    assert_eq!(
        asked("src/main.rs", Some("C:/dev/app/")),
        powershell(r"C:\dev\app\src\main.rs")
    );
    assert_eq!(
        asked(r".\src\main.rs", Some(r"C:\dev\app")),
        powershell(r"C:\dev\app\src\main.rs")
    );
    assert_eq!(
        asked(r"..\x.md", Some(r"C:\dev\app")),
        powershell(r"C:\dev\app\..\x.md")
    );
    assert_eq!(asked("a.md", Some(r"C:\")), powershell(r"C:\a.md"));
}

/// The shell and path a path typed for a host reading `shell` is asked as.
fn typed(shell: RemoteShell, path: &str, cwd: Option<&str>) -> Option<(RemoteShell, String)> {
    RemotePath::typed("me@devbox", shell, path, cwd).map(|remote| (remote.shell, remote.path))
}

#[test]
fn a_typed_home_path_is_read_in_the_host_s_shell() {
    use RemoteShell::{Posix, PowerShell};
    assert_eq!(
        typed(PowerShell, "~/notes.md", None),
        powershell(r"~\notes.md")
    );
    assert_eq!(typed(PowerShell, "~", None), powershell("~"));
    assert_eq!(typed(Posix, "~/notes.md", None), posix("~/notes.md"));
    // A POSIX host expands only a leading `~/`.
    assert_eq!(typed(Posix, r"~\notes.md", None), posix("~/notes.md"));
    assert_eq!(
        typed(Posix, r"~\docs\notes.md", None),
        posix("~/docs/notes.md")
    );
    // The host's shell wins over a cwd of the other form.
    assert_eq!(
        typed(PowerShell, "~/notes.md", Some("/home/me")),
        powershell(r"~\notes.md")
    );
    assert_eq!(typed(PowerShell, "~other/notes.md", None), None);
}

#[test]
fn a_typed_relative_path_joins_a_cwd_in_the_host_s_form() {
    use RemoteShell::{Posix, PowerShell};
    assert_eq!(
        typed(PowerShell, "src/main.rs", Some("C:/dev/app")),
        powershell(r"C:\dev\app\src\main.rs")
    );
    assert_eq!(
        typed(Posix, "./src/main.rs", Some("/home/me/app/")),
        posix("/home/me/app/src/main.rs")
    );
    // No cwd, or one not absolute in the host's form, names no file.
    assert_eq!(typed(PowerShell, "notes.md", None), None);
    assert_eq!(typed(Posix, "notes.md", None), None);
    assert_eq!(typed(PowerShell, "notes.md", Some("/home/me")), None);
    assert_eq!(typed(Posix, "notes.md", Some(r"C:\dev")), None);
    assert_eq!(typed(Posix, "", Some("/home/me")), None);
    // A POSIX cwd that only looks like a `file://` drive path is joined as
    // it reads.
    assert_eq!(
        typed(Posix, "a.md", Some("/C:/Users/me")),
        posix("/C:/Users/me/a.md")
    );
    assert_eq!(
        typed(PowerShell, "a.md", Some("/C:/Users/me")),
        powershell(r"C:\Users\me\a.md")
    );
}

#[test]
fn a_typed_drive_or_unc_path_names_no_file_on_a_posix_host() {
    use RemoteShell::Posix;
    assert_eq!(typed(Posix, "c:/dev/notes.md", None), None);
    assert_eq!(typed(Posix, r"C:\dev\notes.md", Some("/home/me")), None);
    assert_eq!(
        typed(Posix, r"\\srv\share\notes.md", Some("/home/me")),
        None
    );
}

#[test]
fn a_typed_rooted_path_takes_the_drive_of_a_windows_cwd() {
    use RemoteShell::PowerShell;
    assert_eq!(
        typed(PowerShell, "/proj/notes.md", Some(r"D:\proj")),
        powershell(r"D:\proj\notes.md")
    );
    assert_eq!(
        typed(PowerShell, r"\proj\notes.md", Some("d:/work")),
        powershell(r"d:\proj\notes.md")
    );
    assert_eq!(
        typed(PowerShell, "/proj/notes.md", Some("/D:/work")),
        powershell(r"D:\proj\notes.md")
    );
    // With no drive to take, the session's current drive reads it.
    assert_eq!(
        typed(PowerShell, "/proj/notes.md", None),
        powershell(r"\proj\notes.md")
    );
    assert_eq!(
        typed(PowerShell, "/proj/notes.md", Some(r"\\srv\share")),
        powershell(r"\proj\notes.md")
    );
}

#[test]
fn a_typed_absolute_path_is_read_in_the_host_s_shell() {
    use RemoteShell::{Posix, PowerShell};
    assert_eq!(typed(Posix, "/srv/x.md", None), posix("/srv/x.md"));
    assert_eq!(
        typed(PowerShell, "/srv/x.md", Some("/home/me")),
        powershell(r"\srv\x.md")
    );
    assert_eq!(
        typed(PowerShell, r"\dev\x.md", None),
        powershell(r"\dev\x.md")
    );
    assert_eq!(
        typed(PowerShell, "c:/dev/notes.md", None),
        powershell(r"c:\dev\notes.md")
    );
    // A `file://` drive path loses its leading slash, as a printed one does.
    assert_eq!(
        typed(PowerShell, "/C:/Users/me/a.md", None),
        powershell(r"C:\Users\me\a.md")
    );
}

#[test]
fn a_relative_path_without_an_absolute_cwd_has_no_remote_path() {
    assert_eq!(asked("src/main.rs", None), None);
    assert_eq!(asked("src/main.rs", Some("relative/dir")), None);
    assert_eq!(asked("src/main.rs", Some("C:relative")), None);
    assert_eq!(asked("", Some("/home/me")), None);
    assert_eq!(asked(r"\dev\x.md", Some(r"C:\dev")), None);
}
