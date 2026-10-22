//! Opening a folder printed in an SSH pane as a local copy: a probe of the
//! host's tree, the question every transfer asks first, and the fetch,
//! extraction, and opening that follow. Every probe and transfer runs on the
//! background executor; the UI thread only starts them and applies results.

use super::{
    CopyKind, Error, FolderDone, Io, Key, MB, Missing, Owner, RemoteFiles, Result,
    archive::{self, ArchiveFormat},
    basename, cache, meta_of, remove_kept, watch,
};
use crate::window::{Flash, HerdrWindow, RemotePath};
use gpui::Context;
use herdr_client::{RemoteFileError, RemoteShell};
use std::{
    io,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

/// The numbered names a copy kept beside its folder tries before it stays
/// under the name it was moved aside to.
const KEEP_TRIES: u32 = 1000;

/// A question about a folder that waits for the window to have no page open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FolderPrompt {
    pub(crate) remote: RemotePath,
    pub(crate) kind: FolderQuestion,
    /// The probe this question was raised from; both answers that transfer
    /// reuse it, so an answer never probes the host again.
    pub(crate) probed: herdr_client::ProbedFolder,
    /// The working directory a relative path typed in its place is under.
    pub(crate) cwd: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FolderQuestion {
    /// No copy is here yet.
    Copy,
    /// A copy is here and its fingerprint differs from the host's;
    /// `copy_bytes` is what the copy's probe found.
    Changed { copy_bytes: u64 },
}

impl RemoteFiles {
    /// Queues `prompt` unless it already waits.
    pub(crate) fn enqueue_folder(&mut self, prompt: FolderPrompt) {
        if !self.folder_prompts.contains(&prompt) {
            self.folder_prompts.push_back(prompt);
        }
    }

    /// The first waiting folder question, without removing it.
    pub(crate) fn next_folder_prompt(&self) -> Option<FolderPrompt> {
        self.folder_prompts.front().cloned()
    }

    /// Removes the question about `remote`, answered or dismissed.
    pub(crate) fn take_folder_prompt(&mut self, remote: &RemotePath) -> Option<FolderPrompt> {
        let index = self.folder_prompts.iter().position(|prompt| {
            prompt.remote.target == remote.target && prompt.remote.path == remote.path
        })?;
        self.folder_prompts.remove(index)
    }

    /// Ends the in-flight claim on `remote`'s copy when its question is
    /// dismissed, so a later click probes again.
    pub(crate) fn clear_folder_key(&mut self, remote: &RemotePath) {
        self.fetching
            .remove(&(remote.target.clone(), remote.path.clone()));
    }

    /// The uncompressed size the probe found when the folder copy of `key`
    /// was fetched.
    pub(crate) fn folder_bytes(&self, key: &Key) -> Option<u64> {
        match self.copies.get(key)?.kind {
            CopyKind::File => None,
            CopyKind::Folder { bytes, .. } => Some(bytes),
        }
    }

    /// Forgets the copy of `key` and any question about it, as when the
    /// host's path stopped being the kind of thing the copy holds.
    pub(crate) fn forget(&mut self, key: &Key) {
        self.copies.remove(key);
        self.folder_prompts
            .retain(|prompt| prompt.remote.target != key.0 || prompt.remote.path != key.1);
    }
}

impl HerdrWindow {
    /// Probes the host folder `remote` names and opens the copy already here,
    /// or asks whether to copy the folder; a relative path typed in the
    /// question's place is under `cwd`.
    pub(crate) fn open_remote_folder(
        &mut self,
        remote: RemotePath,
        cwd: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let key = (remote.target.clone(), remote.path.clone());
        // The running probe or transfer owns the copy until it ends or the
        // question it asked is dismissed.
        if !self.remote_files.fetching.insert(key) {
            return;
        }
        let name = basename(&remote.path, remote.shell).to_owned();
        self.show_flash(
            Flash::success(format!("Checking {name} on {}…", remote.target)),
            cx,
        );
        let io = self.remote_files.io.clone();
        let owner = self.remote_files.owner;
        let has_copy = self
            .remote_files
            .folder(&remote.target, &remote.path)
            .is_some();
        let task_remote = remote.clone();
        let probe = cx.background_executor().spawn(async move {
            // The UI thread asks no disk whether the copy is still there.
            let present = has_copy
                && (io.cache)().is_ok_and(|cache| {
                    cache
                        .copy_path(
                            owner,
                            &task_remote.target,
                            &task_remote.path,
                            task_remote.shell,
                        )
                        .is_dir()
                });
            (present, (io.folder_probe)(&task_remote))
        });
        cx.spawn(async move |this, cx| {
            let (present, probed) = probe.await;
            // The window may have closed meanwhile; nothing opens then.
            let _ = this.update(cx, |this, cx| {
                this.remote_folder_probed(remote, cwd, present, probed, cx)
            });
        })
        .detach();
    }

    /// Applies a probe of `remote`'s folder: opens the copy that still
    /// matches, or asks whether to copy the folder or fetch it again. A
    /// folder the host does not have asks for another path instead.
    fn remote_folder_probed(
        &mut self,
        remote: RemotePath,
        cwd: Option<String>,
        present: bool,
        probed: herdr_client::Result<herdr_client::ProbedFolder>,
        cx: &mut Context<Self>,
    ) {
        let key = (remote.target.clone(), remote.path.clone());
        let probed = match probed {
            Ok(probed) => probed,
            // The poll opens the question once no page holds the window.
            Err(herdr_client::Error::RemoteFile(RemoteFileError::Missing)) => {
                self.remote_files.fetching.remove(&key);
                tracing::info!(target = %remote.target, "Remote folder not on the host; asking for another path");
                self.remote_files.missing.push_back(Missing {
                    remote,
                    query: None,
                    cwd,
                });
                cx.notify();
                return;
            }
            Err(error) => {
                self.remote_files.fetching.remove(&key);
                // The host's path is a file now: a later click fetches it.
                if matches!(
                    &error,
                    herdr_client::Error::RemoteFile(RemoteFileError::NotAFolder)
                ) {
                    self.remote_files.forget(&key);
                }
                self.show_flash(
                    Flash::warning(crate::Error::from(Error::Probe(error)).to_string()),
                    cx,
                );
                return;
            }
        };
        let copy = self
            .remote_files
            .folder(&remote.target, &remote.path)
            .zip(self.remote_files.folder_bytes(&key))
            .filter(|_| present);
        let kind = match copy {
            // Both fingerprints prove the host's tree unchanged; a host
            // without one asks rather than guess.
            Some(((local, fingerprint), _))
                if fingerprint.is_some() && fingerprint == probed.fingerprint =>
            {
                self.remote_files.fetching.remove(&key);
                self.open_in_system_app(local, None, cx);
                return;
            }
            Some((_, copy_bytes)) => FolderQuestion::Changed { copy_bytes },
            None => FolderQuestion::Copy,
        };
        self.remote_files.enqueue_folder(FolderPrompt {
            remote,
            kind,
            probed,
            cwd,
        });
    }

    /// Copies `remote`'s folder, which `probed` sized, and opens the copy.
    pub(crate) fn start_folder_transfer(
        &mut self,
        remote: RemotePath,
        probed: herdr_client::ProbedFolder,
        cx: &mut Context<Self>,
    ) {
        let name = basename(&remote.path, remote.shell).to_owned();
        let cap = u64::from(self.config.remote_files.folder_transfer_cap_mb) * MB;
        self.show_flash(
            Flash::success(format!(
                "Fetching {name} ({}) from {}…",
                folder_size(probed.bytes),
                remote.target
            )),
            cx,
        );
        let io = self.remote_files.io.clone();
        let owner = self.remote_files.owner;
        let key = (remote.target.clone(), remote.path.clone());
        // A file copy of the same path, from before the host's path became a
        // folder, is replaced only when it still matches this metadata.
        let file_synced = self
            .remote_files
            .copies
            .get(&key)
            .filter(|copy| copy.kind == CopyKind::File)
            .map(|copy| copy.synced_meta);
        let request = Request {
            probed,
            cap_bytes: cap,
            file_synced,
        };
        let fetch = cx
            .background_executor()
            .spawn(async move { fetch_folder(&io, owner, &remote, request) });
        cx.spawn(async move |this, cx| {
            let fetched = fetch.await;
            // The window may have closed meanwhile; nothing opens then.
            let _ = this.update(cx, |this, cx| this.remote_folder_fetched(key, fetched, cx));
        })
        .detach();
    }

    /// Applies a folder copy: keeps it, opens it, and says what it skipped.
    fn remote_folder_fetched(
        &mut self,
        key: Key,
        fetched: Result<FolderFetched>,
        cx: &mut Context<Self>,
    ) {
        self.remote_files.fetching.remove(&key);
        match fetched {
            Ok(fetched) => {
                let local = fetched.done.local.clone();
                let note = finish_note(fetched.links_skipped, fetched.kept.as_deref());
                self.remote_files.record_folder(fetched.done);
                self.open_in_system_app(local, None, cx);
                if let Some(note) = note {
                    self.show_flash(note, cx);
                }
            }
            Err(error) => {
                tracing::warn!(%error, "Remote folder not copied");
                self.show_flash(Flash::warning(crate::Error::from(error).to_string()), cx);
            }
        }
    }
}

/// The flash a finished folder copy shows, if it has anything to say: the
/// links it skipped, and the file copy it kept beside it because that copy
/// may hold edits.
fn finish_note(skipped: u32, kept: Option<&Path>) -> Option<Flash> {
    let skipped = match skipped {
        0 => None,
        1 => Some("1 link was skipped".to_owned()),
        _ => Some(format!("{skipped} links were skipped")),
    };
    let Some(kept) = kept else {
        return skipped.map(Flash::success);
    };
    let name = kept.file_name().unwrap_or_default().to_string_lossy();
    let kept = format!("The file copy's edits were kept as {name}");
    Some(Flash::warning(match skipped {
        Some(skipped) => format!("{kept}; {skipped}"),
        None => kept,
    }))
}

/// A finished folder fetch, ready to record.
struct FolderFetched {
    done: FolderDone,
    links_skipped: u32,
    /// Where an edited file copy of the same path was moved aside.
    kept: Option<PathBuf>,
}

/// What one transfer left at the copy's place.
struct Filled {
    meta: watch::Meta,
    links_skipped: u32,
    kept: Option<PathBuf>,
}

/// What one folder transfer fetches, and what it may replace.
struct Request {
    probed: herdr_client::ProbedFolder,
    cap_bytes: u64,
    /// The synced metadata of a file copy at the same place, when known.
    file_synced: Option<watch::Meta>,
}

/// Where one folder transfer writes, the size limit it fetches under, and
/// the synced metadata of a file copy it may replace.
struct Transfer<'a> {
    archive: &'a Path,
    staging: &'a Path,
    local: &'a Path,
    cap_bytes: u64,
    file_synced: Option<watch::Meta>,
}

/// Fetches `remote`'s folder as an archive and extracts it at `owner`'s copy
/// of the path, replacing any copy there; a file copy that may hold edits is
/// moved aside instead (see [`set_aside`]). Blocking: call only on a
/// background worker.
fn fetch_folder(
    io: &Io,
    owner: Owner,
    remote: &RemotePath,
    request: Request,
) -> Result<FolderFetched> {
    let Request {
        probed,
        cap_bytes,
        file_synced,
    } = request;
    let cache = (io.cache)()?;
    let local = cache.copy_path(owner, &remote.target, &remote.path, remote.shell);
    let parent = local.parent().ok_or_else(|| Error::Cache {
        path: local.clone(),
        source: io::Error::new(io::ErrorKind::InvalidInput, "the copy has no folder"),
    })?;
    cache::create_private_dirs(parent)?;
    // Both calls name a path no other transfer of this process uses.
    let archive = cache::part_path(&local);
    let staging = cache::part_path(&local);
    cache::create_private_dirs(&staging)?;
    let cancelled = AtomicBool::new(false);
    let transfer = Transfer {
        archive: &archive,
        staging: &staging,
        local: &local,
        cap_bytes,
        file_synced,
    };
    let filled = fill(io, remote, &probed.top, transfer, &cancelled);
    let filled = match filled {
        Ok(filled) => filled,
        Err(error) => {
            remove_file(&archive);
            remove_dir(&staging);
            return Err(error);
        }
    };
    Ok(FolderFetched {
        done: FolderDone {
            key: (remote.target.clone(), remote.path.clone()),
            cache,
            local,
            shell: remote.shell,
            meta: filled.meta,
            bytes: probed.bytes,
            fingerprint: probed.fingerprint,
        },
        links_skipped: filled.links_skipped.saturating_add(probed.links_skipped),
        kept: filled.kept,
    })
}

/// Fetches, extracts, and moves the folder into place. `top` is the name the
/// host's archive entries start with. Blocking.
fn fill(
    io: &Io,
    remote: &RemotePath,
    top: &str,
    transfer: Transfer<'_>,
    cancelled: &AtomicBool,
) -> Result<Filled> {
    let mut file = archive::create_file(transfer.archive).map_err(|source| Error::Archive {
        path: transfer.archive.to_owned(),
        source,
    })?;
    let fetched =
        (io.folder_fetch)(remote, &mut file, transfer.cap_bytes).map_err(|error| match error {
            herdr_client::Error::RemoteFile(RemoteFileError::TooLarge { limit }) => {
                Error::FolderTooLarge { limit }
            }
            other => Error::Fetch(other),
        })?;
    drop(file);
    let extracted = archive::extract(
        transfer.archive,
        transfer.staging,
        top,
        format_for(remote.shell),
        cancelled,
    )?;
    // The archive is no use once unpacked; failing to remove it is not.
    remove_file(transfer.archive);
    let kept = if transfer.local.is_dir() {
        std::fs::remove_dir_all(transfer.local).map_err(|source| Error::FolderInUse {
            path: transfer.local.to_owned(),
            source,
        })?;
        None
    } else {
        set_aside(transfer.local, transfer.file_synced)?
    };
    std::fs::rename(transfer.staging, transfer.local).map_err(|source| Error::Cache {
        path: transfer.local.to_owned(),
        source,
    })?;
    let meta = std::fs::metadata(transfer.local)
        .and_then(|meta| watch::Meta::of(&meta))
        .map_err(|source| Error::Cache {
            path: transfer.local.to_owned(),
            source,
        })?;
    tracing::debug!(bytes = fetched.bytes, "Remote folder archive fetched");
    Ok(Filled {
        meta,
        links_skipped: extracted.links_skipped,
        kept,
    })
}

/// Clears the place of a file copy at `local`, from before the host's path
/// became a folder, without losing edits: the copy moves aside first, then
/// is removed only when its metadata proves it the one synced at `synced`.
/// One that may hold edits, or whose synced metadata is not known, stays
/// beside the folder; its path is returned. Blocking.
fn set_aside(local: &Path, synced: Option<watch::Meta>) -> Result<Option<PathBuf>> {
    let kept = cache::kept_path(local);
    match std::fs::rename(local, &kept) {
        Ok(()) => {}
        // No copy, so nothing to keep.
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(Error::FolderInUse {
                path: local.to_owned(),
                source,
            });
        }
    }
    let unedited = match (synced, meta_of(&kept)) {
        (Some(synced), Ok(meta)) => meta == synced,
        (None, _) => false,
        (Some(_), Err(error)) => {
            tracing::warn!(%error, kept = %kept.display(), "Remote file copy moved aside not checked; keeping it");
            false
        }
    };
    if !unedited {
        let kept = keep_beside(&kept, local);
        tracing::warn!(kept = %kept.display(), "A remote file copy that may hold edits was left beside its folder copy");
        return Ok(Some(kept));
    }
    remove_kept(&kept);
    Ok(None)
}

/// Gives the copy moved aside to `moved` the readable name
/// [`cache::kept_name`] makes from `local`'s, beside the folder now at
/// `local`, never in place of a file already there: the first free number
/// wins. The copy stays at `moved` when no name can be had. Blocking.
pub(super) fn keep_beside(moved: &Path, local: &Path) -> PathBuf {
    let name = local.file_name().unwrap_or_default().to_string_lossy();
    for n in 1..=KEEP_TRIES {
        let kept = local.with_file_name(cache::kept_name(&name, n));
        match link_new(moved, &kept) {
            Ok(()) => {
                remove_kept(moved);
                return kept;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                tracing::warn!(%error, kept = %kept.display(), "Remote file copy not renamed beside its folder copy");
                return moved.to_owned();
            }
        }
    }
    tracing::warn!(moved = %moved.display(), "Every readable name for a kept remote file copy is taken");
    moved.to_owned()
}

/// Makes the file at `from` also the file at `to` without replacing one
/// there: a hard link, or a rename once nothing is at `to` where the file
/// system has no hard links.
fn link_new(from: &Path, to: &Path) -> io::Result<()> {
    match std::fs::hard_link(from, to) {
        Err(error) if error.kind() == io::ErrorKind::Unsupported => {
            if std::fs::symlink_metadata(to).is_ok() {
                return Err(io::ErrorKind::AlreadyExists.into());
            }
            std::fs::rename(from, to)
        }
        linked => linked,
    }
}

/// The archive format a host's shell sends a folder in.
fn format_for(shell: RemoteShell) -> ArchiveFormat {
    match shell {
        RemoteShell::Posix => ArchiveFormat::TarGz,
        RemoteShell::PowerShell => ArchiveFormat::Zip,
    }
}

/// `bytes` in the unit that fits once rounded: whole kilobytes, rounded up,
/// under a megabyte; then megabytes, then gigabytes, with one decimal. A
/// value that rounds up to a whole next unit is shown in that unit.
pub(crate) fn folder_size(bytes: u64) -> String {
    const GB: u64 = 1024 * MB;
    let kilobytes = bytes.div_ceil(1024);
    if kilobytes < 1024 {
        return format!("{kilobytes} KB");
    }
    let megabytes = format!("{:.1}", bytes as f64 / MB as f64);
    if bytes < GB && megabytes != "1024.0" {
        return format!("{megabytes} MB");
    }
    format!("{:.1} GB", bytes as f64 / GB as f64)
}

/// `count` files, with a comma between each group of three digits.
pub(crate) fn file_count(count: u64) -> String {
    if count == 1 {
        return "1 file".to_owned();
    }
    let digits = count.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    format!("{grouped} files")
}

fn remove_file(path: &Path) {
    if let Err(error) = std::fs::remove_file(path)
        && error.kind() != io::ErrorKind::NotFound
    {
        tracing::warn!(%error, path = %path.display(), "Remote folder archive not removed");
    }
}

fn remove_dir(path: &Path) {
    if let Err(error) = std::fs::remove_dir_all(path)
        && error.kind() != io::ErrorKind::NotFound
    {
        tracing::warn!(%error, path = %path.display(), "Remote folder staging not removed");
    }
}
