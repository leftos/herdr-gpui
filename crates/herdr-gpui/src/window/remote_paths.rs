//! The path an SSH host reads for a file path one of its panes printed, and
//! the shell it reads it in. A Windows OpenSSH host prints drive and UNC
//! paths with either separator and reads them in PowerShell; any other host
//! reads POSIX paths in `/bin/sh`. Nothing here asks the host anything: `~`
//! and `..` are left for the host to resolve.

use super::file_links::FileLink;
use herdr_client::RemoteShell;

/// A file on an SSH host, as the host's fetch script names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemotePath {
    /// The `[user@]host` SSH target the pane's endpoint runs on.
    pub(crate) target: String,
    pub(crate) shell: RemoteShell,
    /// An absolute path in the shell's own form, or one under `~`.
    pub(crate) path: String,
    /// The line printed after the path.
    pub(crate) line: Option<u32>,
}

impl RemotePath {
    /// The host's file `link` names, when the pane runs on an SSH host and
    /// the path is absolute, under `~`, or relative to an absolute working
    /// directory.
    pub(crate) fn from_link(link: &FileLink) -> Option<Self> {
        let target = link.remote.clone()?;
        let (shell, path) = resolve(&link.path, link.cwd.as_deref())?;
        Some(Self {
            target,
            shell,
            path,
            line: link.line,
        })
    }

    /// The host's file the first row of `link` names alone, when a raw wrap
    /// joined the next row's text to it, resolved as [`Self::from_link`]
    /// resolves the joined path. The line printed belongs to the joined text.
    pub(crate) fn unjoined(link: &FileLink) -> Option<Self> {
        let target = link.remote.clone()?;
        let (shell, path) = resolve(link.unjoined.as_deref()?, link.cwd.as_deref())?;
        Some(Self {
            target,
            shell,
            path,
            line: None,
        })
    }

    /// The host's file a path typed for `target`, whose host reads paths in
    /// `shell`, names. Every typed path is read in `shell`, a relative one
    /// under `cwd`. `None` when it names none, as a relative path does with
    /// no `cwd` absolute in `shell`'s form.
    pub(crate) fn typed(
        target: &str,
        shell: RemoteShell,
        path: &str,
        cwd: Option<&str>,
    ) -> Option<Self> {
        let (shell, path) = resolve_typed(shell, path, cwd)?;
        Some(Self {
            target: target.to_owned(),
            shell,
            path,
            line: None,
        })
    }

    /// The host's file, as the transfer functions name it.
    pub(crate) fn remote_file(&self) -> herdr_client::RemoteFile<'_> {
        herdr_client::RemoteFile {
            target: &self.target,
            shell: self.shell,
            path: &self.path,
        }
    }
}

/// The shell and the absolute or `~` path for `path` as printed under `cwd`.
fn resolve(path: &str, cwd: Option<&str>) -> Option<(RemoteShell, String)> {
    let path = file_url_drive(path);
    if windows(path) {
        return Some((RemoteShell::PowerShell, backslashes(path)));
    }
    if path.starts_with('/') {
        return Some((RemoteShell::Posix, path.to_owned()));
    }
    let cwd = cwd.map(file_url_drive);
    if let Some(rest) = path.strip_prefix('~') {
        let powershell = cwd.map_or(rest.starts_with('\\'), windows);
        let shell = if powershell {
            RemoteShell::PowerShell
        } else {
            RemoteShell::Posix
        };
        return home(shell, path);
    }
    let shell = if cwd.is_some_and(windows) {
        RemoteShell::PowerShell
    } else {
        RemoteShell::Posix
    };
    under(shell, path, cwd?)
}

/// The shell and path for `path` as typed for a host that reads paths in
/// `shell`. PowerShell reads every absolute form, with backslashes so it is
/// keyed as a clicked PowerShell path is; a path rooted without a drive
/// takes the drive of a drive `cwd`. A POSIX host reads no drive or UNC
/// path, so one names no file there.
fn resolve_typed(
    shell: RemoteShell,
    path: &str,
    cwd: Option<&str>,
) -> Option<(RemoteShell, String)> {
    if path.starts_with('~') {
        return home(shell, path);
    }
    match shell {
        RemoteShell::PowerShell => {
            let path = file_url_drive(path);
            let cwd = cwd.map(file_url_drive);
            if windows(path) {
                return Some((shell, backslashes(path)));
            }
            if path.starts_with(['/', '\\']) {
                let drive = cwd.and_then(drive).unwrap_or_default();
                return Some((shell, format!("{drive}{}", backslashes(path))));
            }
            under(shell, path, cwd?)
        }
        RemoteShell::Posix => {
            if windows(path) {
                return None;
            }
            if path.starts_with('/') {
                return Some((shell, path.to_owned()));
            }
            under(shell, path, cwd?)
        }
    }
}

/// The `X:` drive a drive path such as `X:\dev` starts with.
fn drive(path: &str) -> Option<&str> {
    let drive = path.get(..2)?;
    (windows(path) && !path.starts_with('\\')).then_some(drive)
}

/// `path`, which starts with `~`, in `shell`'s form: a POSIX host expands
/// only a leading `~/`, so `~\` is read as `~/` there. `~user` names
/// another account's home, which is not resolved.
fn home(shell: RemoteShell, path: &str) -> Option<(RemoteShell, String)> {
    let rest = path.strip_prefix('~')?;
    if !(rest.is_empty() || rest.starts_with(['/', '\\'])) {
        return None;
    }
    Some(match shell {
        RemoteShell::PowerShell => (shell, backslashes(path)),
        RemoteShell::Posix if rest.starts_with('\\') => (shell, path.replace('\\', "/")),
        RemoteShell::Posix => (shell, path.to_owned()),
    })
}

/// The relative `path` under `cwd`, joined in `shell`'s form. `None` when
/// `path` is empty or relative to the root of a drive no cwd names, or when
/// `cwd` is not absolute in `shell`'s form.
fn under(shell: RemoteShell, path: &str, cwd: &str) -> Option<(RemoteShell, String)> {
    if path.is_empty() || path.starts_with('\\') {
        return None;
    }
    let relative = without_current(path);
    match shell {
        RemoteShell::PowerShell if windows(cwd) => {
            let cwd = backslashes(cwd);
            let folder = cwd.trim_end_matches('\\');
            Some((shell, format!("{folder}\\{}", backslashes(relative))))
        }
        RemoteShell::Posix if cwd.starts_with('/') => {
            let folder = cwd.trim_end_matches('/');
            Some((shell, format!("{folder}/{relative}")))
        }
        _ => None,
    }
}

/// Whether `path` starts with a drive and a separator, as in `C:\` or `c:/`,
/// or is a UNC path such as `\\server\share`.
pub(super) fn windows(path: &str) -> bool {
    path.starts_with(r"\\")
        || matches!(path.as_bytes(), [letter, b':', b'\\' | b'/', ..] if letter.is_ascii_alphabetic())
}

/// A `file://` address's drive path, which reads `/C:/…` on a Unix client,
/// as the drive path `C:/…`; any other path unchanged.
fn file_url_drive(path: &str) -> &str {
    match path.strip_prefix('/') {
        Some(rest) if windows(rest) && !rest.starts_with('\\') => rest,
        _ => path,
    }
}

/// `path` without the `./` or `.\` that only says it is relative.
fn without_current(mut path: &str) -> &str {
    while let Some(rest) = path.strip_prefix("./").or_else(|| path.strip_prefix(r".\")) {
        path = rest;
    }
    path
}

fn backslashes(path: &str) -> String {
    path.replace('/', "\\")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
