//! The question a click on an SSH path asks before fetching when a raw wrap
//! ran the path onto the next row and the first row alone reads as another
//! path: the joined path to open or edit, the first row's part on request,
//! or nothing. It opens only when no other page holds the window.

use super::{
    Page,
    remote_file_dialogs::{buttons, muted, title, typed_choice},
};
use crate::{HerdrWindow, remote_files::RawJoin, search_input::SearchInput};
use gpui::{prelude::*, *};

/// The line under the question's title.
const RAN_ON: &str = "This path ran onto a second line, which may be other output.";

/// The raw-join question: the click it is about, and the field holding the
/// path to open.
pub(in crate::menu) struct JoinDialog {
    join: RawJoin,
    input: Entity<SearchInput>,
}

impl HerdrWindow {
    /// Shows the raw-join question for `join` unless another page holds the
    /// window, its field focused and holding the joined path. `true` when it
    /// opened.
    pub(crate) fn show_remote_join(
        &mut self,
        join: RawJoin,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.menu.page.is_some() || !self.open_menu(window, cx) {
            return false;
        }
        let input = self.remote_path_field(&join.remote.path, window, cx);
        self.menu.remote_join = Some(JoinDialog { join, input });
        self.menu.page = Some(Page::RemoteJoin);
        cx.notify();
        true
    }

    /// The click the open raw-join question is about.
    pub(crate) fn shown_remote_join(&self) -> Option<&RawJoin> {
        if self.menu.page != Some(Page::RemoteJoin) {
            return None;
        }
        self.menu.remote_join.as_ref().map(|dialog| &dialog.join)
    }

    /// Whether `event` belongs to the raw-join question's field, which owns
    /// typing and composition; the question takes only Escape and Enter.
    pub(in crate::menu) fn remote_join_field_key(&self, event: &KeyDownEvent, cx: &App) -> bool {
        self.menu
            .remote_join
            .as_ref()
            .is_some_and(|dialog| dialog.input.read(cx).is_composing())
            || !matches!(event.keystroke.key.as_str(), "escape" | "enter")
    }

    /// Opens the path in the field as a click on a single row opens one,
    /// closing the question. An empty field, or a relative path with no
    /// working directory to put it under, does nothing.
    pub(in crate::menu) fn submit_remote_join(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = &self.menu.remote_join else {
            return;
        };
        let join = &dialog.join;
        let Some((remote, query, cwd)) = typed_choice(
            dialog.input.read(cx),
            &join.remote,
            join.query.as_deref(),
            join.cwd.as_deref(),
        ) else {
            return;
        };
        self.dismiss_menu(window, cx);
        self.open_remote_file(remote, query, None, cwd, cx);
    }

    /// Puts the first row's own path in the field, all selected, and keeps
    /// the field focused; nothing opens until the user does.
    fn offer_first_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = &self.menu.remote_join else {
            return;
        };
        let input = dialog.input.clone();
        let first = dialog.join.unjoined.path.clone();
        input.update(cx, |input, cx| {
            input.set_text_selected(&first, cx);
            window.focus(&input.focus, cx);
        });
        cx.notify();
    }

    /// The path the raw-join question's field holds, while it is open.
    #[cfg(test)]
    pub(crate) fn remote_join_text(&self, cx: &App) -> Option<String> {
        let dialog = self.menu.remote_join.as_ref()?;
        Some(dialog.input.read(cx).text().to_owned())
    }

    /// The raw-join question's title and the line under it, while it is
    /// open.
    #[cfg(test)]
    pub(crate) fn remote_join_words(&self) -> Option<(String, &'static str)> {
        let dialog = self.menu.remote_join.as_ref()?;
        Some((join_title(&dialog.join), RAN_ON))
    }

    pub(in crate::menu) fn render_remote_join(&self, cx: &mut Context<Self>) -> Div {
        let Some(dialog) = &self.menu.remote_join else {
            return div();
        };
        div()
            .debug_selector(|| "remote-join".into())
            .child(title(join_title(&dialog.join)))
            .child(muted(self.theme.muted, RAN_ON))
            .child(div().p(px(8.)).child(dialog.input.clone()))
            .child(
                buttons()
                    .child(
                        self.remote_file_button("remote-join-open", "Open", true)
                            .on_click(cx.listener(|this, _, window, cx| {
                                cx.stop_propagation();
                                this.submit_remote_join(window, cx);
                            })),
                    )
                    .child(
                        self.remote_file_button("remote-join-first-line", "First line only", false)
                            .on_click(cx.listener(|this, _, window, cx| {
                                cx.stop_propagation();
                                this.offer_first_line(window, cx);
                            })),
                    )
                    .child(
                        self.remote_file_button("remote-join-cancel", "Cancel", false)
                            .on_click(cx.listener(|this, _, window, cx| {
                                cx.stop_propagation();
                                this.dismiss_menu(window, cx);
                            })),
                    ),
            )
    }
}

/// The question's title, naming the host as the not-on-host question does.
fn join_title(join: &RawJoin) -> String {
    format!("Open from {}?", join.remote.target)
}
