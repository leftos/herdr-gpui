use super::*;
use std::path::Path;

mod folder;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod folder_exec;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod folder_size;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::fs::PermissionsExt;

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod posix_exec;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod posix_setgid;
mod powershell_exec;
mod scripts;

/// A file name every shell would reinterpret if it were not quoted: spaces,
/// both kinds of single quote, `$`, a backtick, non-ASCII, and a leading `-`.
const HOSTILE_NAME: &str = "-a b'c $x `y \u{e9}\u{2019}.bin";

/// Bytes a text-mode stream would mangle, then a deterministic noise tail.
fn payload(len: usize) -> Vec<u8> {
    let mut bytes = vec![0x00, 0x0A, 0x0D, 0xFF, 0x0D, 0x0A, 0x1A, 0x00];
    let mut state: u32 = 0x1234_5678;
    while bytes.len() < len {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        bytes.push(state.to_be_bytes()[0]);
    }
    bytes.truncate(len);
    bytes
}

/// The bytes in [`small_tree`]'s regular files: 6 + 13 + 13.
const SMALL_TREE_BYTES: u64 = 32;

/// Makes a folder at `root` of three small files, one of them in a
/// subfolder, and on Unix a link to one of them, which no probe counts.
fn small_tree(root: &Path) {
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("a.txt"), b"alpha\n").unwrap();
    std::fs::write(root.join("b.md"), b"# notes\nbody\n").unwrap();
    std::fs::write(root.join("sub").join("c.rs"), b"fn main() {}\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("../a.txt", root.join("sub").join("link")).unwrap();
}

/// The script failure inside `error`, if that is what it is.
fn script_error(error: &Error) -> Option<&crate::Error> {
    match error {
        Error::Script(inner) => Some(inner.as_ref()),
        _ => None,
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// The names in `dir`, sorted, so a leftover temporary file shows up.
fn entries(dir: &Path) -> Vec<String> {
    let mut names = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    names.sort();
    names
}

/// `script` as `-EncodedCommand` expects it: UTF-16LE, then base64.
fn encode(script: &str) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let utf16 = script
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    STANDARD.encode(utf16)
}

fn decode(encoded: &str) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let bytes = STANDARD.decode(encoded).unwrap();
    let units = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&units).unwrap()
}

/// Runs the POSIX scripts under this machine's `/bin/sh` through the same
/// runner an SSH host's scripts use.
#[cfg(any(target_os = "linux", target_os = "macos"))]
struct Fixture {
    home: tempfile::TempDir,
    /// The child's `PATH`; `None` keeps this process's.
    path_env: Option<std::path::PathBuf>,
    /// The umask the script starts with, as an SSH login sets one.
    umask: &'static str,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Fixture {
    fn new() -> Self {
        Self {
            home: tempfile::tempdir().unwrap(),
            path_env: None,
            umask: "022",
        }
    }

    fn mode(&self, name: &str) -> u32 {
        std::fs::metadata(self.path(name))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777
    }

    fn set_mode(&self, name: &str, mode: u32) {
        std::fs::set_permissions(self.path(name), std::fs::Permissions::from_mode(mode)).unwrap();
    }

    fn path(&self, name: &str) -> std::path::PathBuf {
        self.home.path().join(name)
    }

    fn text(&self, name: &str) -> String {
        self.path(name).to_str().unwrap().to_owned()
    }

    fn command(&self, script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", &format!("umask {}\n{script}", self.umask)])
            .env("HOME", self.home.path());
        if let Some(path) = &self.path_env {
            command.env("PATH", path);
        }
        command
    }

    fn fetch(&self, path: &str, max_bytes: u64) -> std::result::Result<(Vec<u8>, Fetched), Error> {
        let mut out = Vec::new();
        let command = self.command(&posix::fetch_script(path, max_bytes));
        let fetched = fetch_with(command, &mut out, max_bytes, &AtomicBool::new(false))?;
        Ok((out, fetched))
    }

    fn push(&self, path: &str, expect: Expect, input: &[u8]) -> std::result::Result<(), Error> {
        self.push_claiming(path, expect, input.len() as u64, input)
    }

    /// A push that says it sends `len` bytes and sends `input`.
    fn push_claiming(
        &self,
        path: &str,
        expect: Expect,
        len: u64,
        input: &[u8],
    ) -> std::result::Result<(), Error> {
        let command = self.command(&posix::push_script(path, expect, Access::Keep, len));
        push_with(command, input, &AtomicBool::new(false))
    }

    /// A push of a new file at `path` that takes the access of `source`, as
    /// a kept-both copy does.
    fn push_like(&self, path: &str, source: &str, input: &[u8]) -> std::result::Result<(), Error> {
        let script = posix::push_script(
            path,
            Expect::Absent,
            Access::Like(source),
            input.len() as u64,
        );
        push_with(self.command(&script), input, &AtomicBool::new(false))
    }

    /// Runs the scripts with a `chgrp` that always fails first on `PATH`, as
    /// for a user outside the file's group. Keep the returned folder alive.
    fn fail_chgrp(&mut self) -> tempfile::TempDir {
        self.shim("chgrp", "exit 1\n")
    }

    /// Runs the scripts with a `/bin/sh` script named `tool` holding `body`
    /// first on `PATH`. Keep the returned folder alive. The shim is written
    /// from a child shell, so no sibling test's fork can hold it open for
    /// writing when the script runs it.
    fn shim(&mut self, tool: &str, body: &str) -> tempfile::TempDir {
        let bin = tempfile::tempdir().unwrap();
        let shim = bin.path().join(tool);
        crate::test_executable::write(&shim, format!("#!/bin/sh\n{body}"), 0o755).unwrap();
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut folders = vec![bin.path().to_owned()];
        folders.extend(std::env::split_paths(&path));
        self.path_env = Some(std::env::join_paths(folders).unwrap().into());
        bin
    }
}

/// Where this machine keeps `tool`.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn real_tool(tool: &str) -> std::path::PathBuf {
    ["/bin", "/usr/bin"]
        .iter()
        .map(|folder| Path::new(folder).join(tool))
        .find(|candidate| candidate.exists())
        .unwrap()
}
