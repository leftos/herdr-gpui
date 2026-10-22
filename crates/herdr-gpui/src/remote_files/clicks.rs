//! A click on an SSH path: the raw-join question it may ask first, the
//! cached folder or file copy it reopens, the fetch it starts, and what
//! the fetch's result opens or asks.

use super::{Error, Fetch, RawJoin, Result, basename, fetch_copy};
use crate::window::{Flash, HerdrWindow, RemotePath};
use gpui::Context;

/// A click's file the host does not have, waiting for the window to have
/// no page open to ask for another path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Missing {
    /// The host's file the click asked for.
    pub(crate) remote: RemotePath,
    /// The `?query` or `#fragment` the click's link printed after the path.
    pub(crate) query: Option<String>,
    /// The working directory a relative path typed in its place is under.
    pub(crate) cwd: Option<String>,
}

/// What a click asked for besides the host's file.
struct Click {
    query: Option<String>,
    cwd: Option<String>,
}

impl HerdrWindow {
    /// Fetches the host's file `remote` names into the cache and opens the
    /// copy with the system's default application, or says why it could not.
    /// A copy in conflict is reopened, and its question asked again. A page
    /// opens with `query`, the `?query` or `#fragment` printed after its path.
    /// `unjoined` is the path the first row reads alone, where a raw wrap
    /// joined the next row's text to `remote`: such a click fetches nothing
    /// and queues the question of which path to open. A file the host does
    /// not have queues the question that offers its path to edit; a relative
    /// path typed in either is under `cwd`.
    pub(crate) fn open_remote_file(
        &mut self,
        remote: RemotePath,
        query: Option<String>,
        unjoined: Option<RemotePath>,
        cwd: Option<String>,
        cx: &mut Context<Self>,
    ) {
        // Asked before anything else, a cached folder copy included.
        if let Some(unjoined) = unjoined {
            let join = RawJoin {
                remote,
                query,
                unjoined,
                cwd,
            };
            self.ask_raw_join(join, cx);
            return;
        }
        // A folder copy goes to its probe, never through a file fetch.
        if self
            .remote_files
            .folder(&remote.target, &remote.path)
            .is_some()
        {
            self.open_remote_folder(remote, cwd, cx);
            return;
        }
        if !self.remote_files.fetch_again(&remote) {
            let shown = self.shown_remote_file_prompt();
            let key = (remote.target.clone(), remote.path.clone());
            self.remote_files.reask_conflict(key, shown.as_ref());
            if let Some(copy) = self.remote_files.copy(&remote.target, &remote.path) {
                self.open_in_system_app(copy.local.clone(), query, cx);
            }
            return;
        }
        let key = (remote.target.clone(), remote.path.clone());
        // The fetch already running opens the copy when it is done.
        if !self.remote_files.fetching.insert(key.clone()) {
            return;
        }
        let name = basename(&remote.path, remote.shell);
        self.show_flash(
            Flash::success(format!("Fetching {name} from {}…", remote.target)),
            cx,
        );
        let io = self.remote_files.io.clone();
        let owner = self.remote_files.owner;
        let synced = self
            .remote_files
            .copy(&remote.target, &remote.path)
            .map(|copy| copy.synced_meta);
        let page_query = query.clone();
        let asked = remote.clone();
        let fetch = cx
            .background_executor()
            .spawn(async move { fetch_copy(&io, owner, &asked, synced, page_query.as_deref()) });
        let click = Click { query, cwd };
        cx.spawn(async move |this, cx| {
            let fetched = fetch.await;
            // The window may have closed meanwhile; nothing opens then.
            let _ = this.update(cx, |this, cx| {
                this.remote_file_fetched(remote, click, fetched, cx)
            });
        })
        .detach();
    }

    fn remote_file_fetched(
        &mut self,
        remote: RemotePath,
        click: Click,
        fetched: Result<Fetch>,
        cx: &mut Context<Self>,
    ) {
        let key = (remote.target.clone(), remote.path.clone());
        self.remote_files.fetching.remove(&key);
        match fetched {
            // Saved while the fetch ran: the copy with the save opens, and
            // the watch takes the save from there.
            Ok(Fetch::KeptEdits) => {
                if let Some(copy) = self.remote_files.copies.get(&key) {
                    self.open_in_system_app(copy.local.clone(), click.query, cx);
                }
            }
            Ok(Fetch::Written(done)) => {
                let opened = done.opened.clone();
                self.remote_files.record(*done);
                match opened {
                    Some(url) => self.open_file_url(url, cx),
                    None => tracing::warn!("Fetched remote file copy could not be opened"),
                }
            }
            // The host's path is a folder, so the click becomes a folder copy.
            Err(error)
                if matches!(
                    &error,
                    Error::Fetch(herdr_client::Error::RemoteFile(
                        herdr_client::RemoteFileError::NotAFile
                    ))
                ) =>
            {
                self.open_remote_folder(remote, click.cwd, cx);
            }
            // The poll opens the question once no page holds the window.
            Err(error) if error.is_missing() => {
                tracing::info!(target = %remote.target, "Remote file not on the host; asking for another path");
                self.remote_files.missing.push_back(Missing {
                    remote,
                    query: click.query,
                    cwd: click.cwd,
                });
                cx.notify();
            }
            Err(error) => {
                tracing::warn!(%error, "Remote file not fetched");
                self.show_flash(Flash::warning(crate::Error::from(error).to_string()), cx);
            }
        }
    }
}
