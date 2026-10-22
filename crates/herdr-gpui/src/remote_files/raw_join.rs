//! The question a click on an SSH path asks when a raw wrap ran the path
//! onto the next row and the first row alone reads as another path. It is
//! asked before anything is fetched, so the joined reading never opens a
//! different file on the host without the user's say.

use crate::window::{HerdrWindow, RemotePath};
use gpui::{Context, Window};

/// A raw-joined click, waiting for the window to have no page open to ask
/// which path it meant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawJoin {
    /// The joined path the click read, with its line.
    pub(crate) remote: RemotePath,
    /// The `?query` or `#fragment` the click's link printed after the path.
    pub(crate) query: Option<String>,
    /// The path the first row reads alone.
    pub(crate) unjoined: RemotePath,
    /// The working directory a relative path typed in its place is under.
    pub(crate) cwd: Option<String>,
}

impl RawJoin {
    /// Whether this question is about the same joined host path as `other`.
    fn asks_about(&self, other: &RemotePath) -> bool {
        self.remote.target == other.target && self.remote.path == other.path
    }
}

impl HerdrWindow {
    /// Queues the question for `join` unless one about the same joined path
    /// is already waiting or open; the poll opens it once no page holds the
    /// window.
    pub(super) fn ask_raw_join(&mut self, join: RawJoin, cx: &mut Context<Self>) {
        let pending = |asked: &RawJoin| asked.asks_about(&join.remote);
        if self.remote_files.raw_joins.iter().any(pending)
            || self.shown_remote_join().is_some_and(pending)
        {
            return;
        }
        tracing::info!(target = %join.remote.target, "Remote path ran onto the next row; asking which to open");
        self.remote_files.raw_joins.push_back(join);
        cx.notify();
    }

    /// Opens the first waiting raw-join question. `true` when one was
    /// waiting, opened or not, so no question queued after it opens first.
    pub(super) fn show_next_raw_join(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(join) = self.remote_files.raw_joins.front().cloned() else {
            return false;
        };
        if self.show_remote_join(join, window, cx) {
            self.remote_files.raw_joins.pop_front();
        }
        true
    }
}
