//! The questions a save to a remote file's copy asks: whether to upload it,
//! and what to do when the host's file changed since it was synced; and the
//! one a click's file the host does not have asks: which path to open
//! instead; and the one every folder transfer asks first. They open only
//! when no other page holds the window, and wait until then.

use super::Page;
use crate::{
    HerdrWindow,
    fonts::StyledFont,
    remote_files::{
        Answer, FolderPrompt, FolderQuestion, Missing, Prompt, PromptKind, Resolution, basename,
        file_count, folder_size,
    },
    search_input::SearchInput,
    window::RemotePath,
};
use gpui::{prelude::*, *};
use std::ops::Range;

/// The not-on-host question: the click's missing file, and the field holding
/// the path to open instead.
pub(in crate::menu) struct MissingDialog {
    missing: Missing,
    input: Entity<SearchInput>,
}

/// The folder question: the probe it asks about, and the field holding the
/// path to copy.
pub(in crate::menu) struct FolderDialog {
    prompt: FolderPrompt,
    input: Entity<SearchInput>,
}

impl HerdrWindow {
    /// Shows the not-on-host question for `missing` unless another page holds
    /// the window, its field focused and holding the path to edit. `true`
    /// when it opened.
    pub(crate) fn show_remote_missing(
        &mut self,
        missing: Missing,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.menu.page.is_some() || !self.open_menu(window, cx) {
            return false;
        }
        let input = self.remote_path_field(&missing.remote.path, window, cx);
        self.menu.remote_missing = Some(MissingDialog { missing, input });
        self.menu.page = Some(Page::RemoteMissing);
        cx.notify();
        true
    }

    /// A focused field for a path on the host, holding `text` all selected
    /// so typing replaces it.
    pub(in crate::menu) fn remote_path_field(
        &self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<SearchInput> {
        let input = cx.new(SearchInput::new);
        input.update(cx, |input, cx| {
            input.set_appearance(self.config.ui.clone(), self.theme.clone(), cx);
            input.set_placeholder("Path on the host", cx);
            input.set_text_selected(text, cx);
            window.focus(&input.focus, cx);
        });
        input
    }

    /// Whether `event` belongs to the open question's path field, which
    /// owns typing and composition; the question takes only Escape and Enter.
    pub(in crate::menu) fn remote_path_field_key(&self, event: &KeyDownEvent, cx: &App) -> bool {
        let input = match self.menu.page {
            Some(Page::RemoteMissing) => self.menu.remote_missing.as_ref().map(|d| &d.input),
            Some(Page::RemoteFolder) => self.menu.remote_folder.as_ref().map(|d| &d.input),
            _ => None,
        };
        input.is_some_and(|input| input.read(cx).is_composing())
            || !matches!(event.keystroke.key.as_str(), "escape" | "enter")
    }

    /// Opens the path in the field as a click opens one, closing the
    /// question. An empty field, or a relative path with no working
    /// directory to put it under, does nothing.
    pub(in crate::menu) fn submit_remote_missing(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((remote, query, cwd)) = self.remote_missing_choice(cx) else {
            return;
        };
        self.dismiss_menu(window, cx);
        self.open_remote_file(remote, query, None, cwd, cx);
    }

    fn remote_missing_choice(
        &self,
        cx: &App,
    ) -> Option<(RemotePath, Option<String>, Option<String>)> {
        let dialog = self.menu.remote_missing.as_ref()?;
        let missing = &dialog.missing;
        typed_choice(
            dialog.input.read(cx),
            &missing.remote,
            missing.query.as_deref(),
            missing.cwd.as_deref(),
        )
    }

    /// The path the not-on-host question's field holds, while it is open.
    #[cfg(test)]
    pub(crate) fn remote_missing_text(&self, cx: &App) -> Option<String> {
        let dialog = self.menu.remote_missing.as_ref()?;
        Some(dialog.input.read(cx).text().to_owned())
    }

    /// The open folder question's title, path, and facts lines, as it shows
    /// them.
    #[cfg(test)]
    pub(crate) fn remote_folder_text(&self) -> Option<Vec<String>> {
        let prompt = &self.menu.remote_folder.as_ref()?.prompt;
        let text = FolderText::of(prompt);
        let mut lines = vec![text.title, prompt.remote.path.clone(), text.facts];
        lines.extend(text.copy);
        Some(lines)
    }

    /// The host path a question is about, read-only.
    fn remote_path_box(&self, id: &'static str, path: &str) -> Div {
        div().px(px(8.)).child(
            div()
                .debug_selector(move || id.into())
                .rounded(px(crate::config::corners::CONTROL))
                .bg(rgb(self.theme.active))
                .px(px(10.))
                .py(px(6.))
                .overflow_hidden()
                .text_font(&self.config.terminal)
                .child(path.to_owned()),
        )
    }

    pub(in crate::menu) fn render_remote_missing(&self, cx: &mut Context<Self>) -> Div {
        let Some(dialog) = &self.menu.remote_missing else {
            return div();
        };
        let remote = &dialog.missing.remote;
        div()
            .debug_selector(|| "remote-missing".into())
            .child(title(format!("Not on {}", remote.target)))
            .child(self.remote_path_box("remote-missing-path", &remote.path))
            .child(div().p(px(8.)).child(dialog.input.clone()))
            .child(
                buttons()
                    .child(
                        self.remote_file_button("remote-missing-open", "Open", true)
                            .on_click(cx.listener(|this, _, window, cx| {
                                cx.stop_propagation();
                                this.submit_remote_missing(window, cx);
                            })),
                    )
                    .child(
                        self.remote_file_button("remote-missing-cancel", "Cancel", false)
                            .on_click(cx.listener(|this, _, window, cx| {
                                cx.stop_propagation();
                                this.dismiss_menu(window, cx);
                            })),
                    ),
            )
    }

    /// The question the open page asks, if it is one of these.
    pub(crate) fn shown_remote_file_prompt(&self) -> Option<Prompt> {
        let kind = match self.menu.page? {
            Page::RemoteUpload => PromptKind::Upload,
            Page::RemoteConflict => PromptKind::Conflict,
            _ => return None,
        };
        Some(Prompt {
            key: self.menu.remote_file.clone()?,
            kind,
        })
    }

    /// Shows `prompt` unless another page holds the window. `true` when it
    /// opened.
    pub(crate) fn show_remote_file_prompt(
        &mut self,
        prompt: &Prompt,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.menu.page.is_some() || !self.open_menu(window, cx) {
            return false;
        }
        self.menu.remote_file = Some(prompt.key.clone());
        self.menu.page = Some(match prompt.kind {
            PromptKind::Upload => Page::RemoteUpload,
            PromptKind::Conflict => Page::RemoteConflict,
        });
        true
    }

    /// Closes the open question without answering it: its copy no longer
    /// waits for the answer. The copy is forgotten first, so closing is not
    /// taken as a "Not now".
    pub(crate) fn close_remote_file_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu.remote_file = None;
        self.dismiss_menu(window, cx);
    }

    /// Closing the upload question without an answer is a "Not now".
    pub(in crate::menu) fn decline_remote_upload_on_dismiss(&mut self, cx: &mut Context<Self>) {
        if self.menu.page == Some(Page::RemoteUpload)
            && let Some(key) = self.menu.remote_file.take()
        {
            self.answer_remote_upload(key, Answer::NotNow, cx);
        }
    }

    pub(in crate::menu) fn confirm_remote_upload(
        &mut self,
        answer: Answer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = self.menu.remote_file.take();
        self.dismiss_menu(window, cx);
        if let Some(key) = key {
            self.answer_remote_upload(key, answer, cx);
        }
    }

    pub(in crate::menu) fn confirm_remote_conflict(
        &mut self,
        resolution: Resolution,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = self.menu.remote_file.take();
        self.dismiss_menu(window, cx);
        if let Some(key) = key {
            self.resolve_remote_conflict(key, resolution, cx);
        }
    }

    pub(in crate::menu) fn render_remote_upload(&self, cx: &mut Context<Self>) -> Div {
        let Some((target, path)) = &self.menu.remote_file else {
            return div();
        };
        let Some(copy) = self.remote_files.copy(target, path) else {
            return div();
        };
        let theme = &self.theme;
        div()
            .debug_selector(|| "remote-upload".into())
            .child(title(format!(
                "Upload your changes to {}?",
                basename(path, copy.shell)
            )))
            .child(
                div().px(px(8.)).child(
                    div()
                        .debug_selector(|| "remote-upload-path".into())
                        .rounded(px(crate::config::corners::CONTROL))
                        .bg(rgb(theme.active))
                        .px(px(10.))
                        .py(px(6.))
                        .overflow_hidden()
                        .text_font(&self.config.terminal)
                        .child(div().child(target.clone()))
                        .child(div().child(path.clone())),
                ),
            )
            .child(muted(
                theme.muted,
                "Your saved copy replaces the file on the host.",
            ))
            .child(
                buttons()
                    .child(self.upload_button("remote-upload-yes", "Upload", Answer::Upload, cx))
                    .child(self.upload_button(
                        "remote-upload-always",
                        "Upload future saves without asking",
                        Answer::Always,
                        cx,
                    ))
                    .child(
                        self.remote_file_button("remote-upload-no", "Not now", false)
                            .on_click(cx.listener(|this, _, window, cx| {
                                cx.stop_propagation();
                                this.dismiss_menu(window, cx);
                            })),
                    ),
            )
    }

    pub(in crate::menu) fn render_remote_conflict(&self, cx: &mut Context<Self>) -> Div {
        let Some((target, path)) = &self.menu.remote_file else {
            return div();
        };
        let Some(copy) = self.remote_files.copy(target, path) else {
            return div();
        };
        div()
            .debug_selector(|| "remote-conflict".into())
            .child(title(format!(
                "{} changed on {target}",
                basename(path, copy.shell)
            )))
            .child(muted(
                self.theme.muted,
                "It changed since it was fetched, so your save was not uploaded.",
            ))
            .child(
                buttons()
                    .child(self.conflict_button(
                        "remote-conflict-keep-both",
                        "Keep both",
                        Resolution::KeepBoth,
                        cx,
                    ))
                    .child(self.conflict_button(
                        "remote-conflict-overwrite",
                        "Overwrite anyway",
                        Resolution::Overwrite,
                        cx,
                    ))
                    .child(self.conflict_button(
                        "remote-conflict-refetch",
                        "Re-fetch",
                        Resolution::Refetch,
                        cx,
                    )),
            )
    }

    /// Shows the folder question `prompt` unless another page holds the
    /// window, its field focused and holding the folder's path. `true` when
    /// it opened.
    pub(crate) fn show_remote_folder_prompt(
        &mut self,
        prompt: &FolderPrompt,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.menu.page.is_some() || !self.open_menu(window, cx) {
            return false;
        }
        let input = self.remote_path_field(&prompt.remote.path, window, cx);
        self.menu.remote_folder = Some(FolderDialog {
            prompt: prompt.clone(),
            input,
        });
        self.menu.page = Some(Page::RemoteFolder);
        cx.notify();
        true
    }

    /// Closing the folder question without an answer transfers nothing, and
    /// lets a later click probe again.
    pub(in crate::menu) fn decline_remote_folder_on_dismiss(&mut self) {
        if let Some(dialog) = self.menu.remote_folder.take() {
            self.remote_files.clear_folder_key(&dialog.prompt.remote);
        }
    }

    /// Answers the folder question with the button Enter presses: Copy for
    /// a first fetch, Open existing for a changed folder.
    pub(in crate::menu) fn submit_remote_folder(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = &self.menu.remote_folder else {
            return;
        };
        let answer = match dialog.prompt.kind {
            FolderQuestion::Copy => FolderAnswer::Transfer,
            FolderQuestion::Changed { .. } => FolderAnswer::Open,
        };
        self.confirm_remote_folder(answer, window, cx);
    }

    /// Answers the folder question the window shows. While the field holds
    /// the folder's path, the answer reuses the probe it was raised from, so
    /// the host is never probed again; any other path is opened as a click
    /// opens one, and asks its own question. An empty field, or a relative
    /// path with no working directory to put it under, does nothing.
    pub(in crate::menu) fn confirm_remote_folder(
        &mut self,
        answer: FolderAnswer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if answer == FolderAnswer::Cancel {
            self.dismiss_menu(window, cx);
            return;
        }
        let Some(choice) = self.remote_folder_choice(cx) else {
            return;
        };
        let Some(dialog) = self.menu.remote_folder.take() else {
            return;
        };
        let prompt = dialog.prompt;
        let Some(typed) = choice else {
            self.dismiss_menu(window, cx);
            self.answer_remote_folder(prompt, answer, cx);
            return;
        };
        self.remote_files.clear_folder_key(&prompt.remote);
        self.dismiss_menu(window, cx);
        self.open_remote_file(typed, None, None, prompt.cwd, cx);
    }

    /// What the field asks for: `None` while it holds nothing to open,
    /// `Some(None)` while it holds the folder's own path, else the typed path.
    fn remote_folder_choice(&self, cx: &App) -> Option<Option<RemotePath>> {
        let dialog = self.menu.remote_folder.as_ref()?;
        let input = dialog.input.read(cx);
        let text = input.text().trim();
        if input.is_composing() || text.is_empty() {
            return None;
        }
        let prompt = &dialog.prompt;
        if text == prompt.remote.path {
            return Some(None);
        }
        let remote = &prompt.remote;
        RemotePath::typed(&remote.target, remote.shell, text, prompt.cwd.as_deref()).map(Some)
    }

    /// Transfers the folder `prompt` asked about, or opens its copy.
    fn answer_remote_folder(
        &mut self,
        prompt: FolderPrompt,
        answer: FolderAnswer,
        cx: &mut Context<Self>,
    ) {
        match answer {
            FolderAnswer::Transfer => {
                self.start_folder_transfer(prompt.remote, prompt.probed, cx);
            }
            FolderAnswer::Open => {
                if let Some((local, _)) = self
                    .remote_files
                    .folder(&prompt.remote.target, &prompt.remote.path)
                {
                    self.open_in_system_app(local, None, cx);
                }
                self.remote_files.clear_folder_key(&prompt.remote);
            }
            FolderAnswer::Cancel => self.remote_files.clear_folder_key(&prompt.remote),
        }
    }

    pub(in crate::menu) fn render_remote_folder(&self, cx: &mut Context<Self>) -> Div {
        let Some(dialog) = &self.menu.remote_folder else {
            return div();
        };
        let prompt = &dialog.prompt;
        let text = FolderText::of(prompt);
        let theme = &self.theme;
        let strong = HighlightStyle {
            color: Some(rgb(theme.foreground).into()),
            font_weight: Some(FontWeight::SEMIBOLD),
            ..Default::default()
        };
        let facts = StyledText::new(text.facts)
            .with_highlights(text.highlights.map(|range| (range, strong)));
        let cancel = self.folder_button(
            "remote-folder-cancel",
            "Cancel",
            false,
            FolderAnswer::Cancel,
            cx,
        );
        div()
            .debug_selector(|| "remote-folder".into())
            .child(title(text.title))
            .child(self.remote_path_box("remote-folder-path", &prompt.remote.path))
            .child(div().p(px(8.)).child(dialog.input.clone()))
            .child(
                div()
                    .p(px(8.))
                    .text_color(rgb(theme.muted))
                    .child(div().child(facts))
                    .when_some(text.copy, |lines, copy| lines.child(div().child(copy))),
            )
            .child(match prompt.kind {
                FolderQuestion::Copy => buttons()
                    .child(self.folder_button(
                        "remote-folder-copy",
                        "Copy",
                        true,
                        FolderAnswer::Transfer,
                        cx,
                    ))
                    .child(cancel),
                FolderQuestion::Changed { .. } => buttons()
                    .child(self.folder_button(
                        "remote-folder-refetch",
                        "Re-fetch",
                        false,
                        FolderAnswer::Transfer,
                        cx,
                    ))
                    .child(self.folder_button(
                        "remote-folder-open",
                        "Open existing",
                        true,
                        FolderAnswer::Open,
                        cx,
                    ))
                    .child(cancel),
            })
    }

    fn folder_button(
        &self,
        id: &'static str,
        label: &'static str,
        primary: bool,
        answer: FolderAnswer,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.remote_file_button(id, label, primary)
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.confirm_remote_folder(answer, window, cx);
            }))
    }

    pub(in crate::menu) fn remote_file_button(
        &self,
        id: &'static str,
        label: &'static str,
        primary: bool,
    ) -> Stateful<Div> {
        let active = self.theme.active;
        div()
            .id(id)
            .debug_selector(move || id.into())
            .p(px(8.))
            .rounded(px(crate::config::corners::CONTROL))
            .when(primary, |button| button.bg(rgb(active)))
            .when(!primary, |button| button.hover(|s| s.bg(rgb(active))))
            .cursor_pointer()
            .child(label)
    }

    fn upload_button(
        &self,
        id: &'static str,
        label: &'static str,
        answer: Answer,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.remote_file_button(id, label, answer == Answer::Upload)
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.confirm_remote_upload(answer, window, cx);
            }))
    }

    fn conflict_button(
        &self,
        id: &'static str,
        label: &'static str,
        resolution: Resolution,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.remote_file_button(id, label, resolution == Resolution::KeepBoth)
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.confirm_remote_conflict(resolution, window, cx);
            }))
    }
}

/// The answers a folder question takes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::menu) enum FolderAnswer {
    Cancel,
    Open,
    Transfer,
}

/// What the folder question says: its title, its facts line and the ranges
/// of it that stand out, and for a changed folder the size of the copy here.
struct FolderText {
    title: String,
    facts: String,
    highlights: [Range<usize>; 2],
    copy: Option<String>,
}

impl FolderText {
    fn of(prompt: &FolderPrompt) -> Self {
        let size = folder_size(prompt.probed.bytes);
        let count = file_count(prompt.probed.files);
        let target = &prompt.remote.target;
        let (title, lead, copy) = match prompt.kind {
            FolderQuestion::Copy => (format!("Copy this folder from {target}?"), "", None),
            FolderQuestion::Changed { copy_bytes } => (
                format!(
                    "{} changed on {target}",
                    basename(&prompt.remote.path, prompt.remote.shell)
                ),
                "Now ",
                Some(format!("Your copy: {}", folder_size(copy_bytes))),
            ),
        };
        let size_at = lead.len();
        let count_at = size_at + size.len() + " in ".len();
        let highlights = [
            size_at..size_at + size.len(),
            count_at..count_at + count.len(),
        ];
        Self {
            title,
            facts: format!("{lead}{size} in {count}, before compression"),
            highlights,
            copy,
        }
    }
}

/// The file a path field names for the clicked file `clicked`, the query it
/// opens with, and the working directory it was resolved under. The click's
/// `query` and line stay only while the field still holds the clicked path.
/// An empty field, one mid-composition, or a relative path with no `cwd` to
/// put it under names none.
pub(super) fn typed_choice(
    input: &SearchInput,
    clicked: &RemotePath,
    query: Option<&str>,
    cwd: Option<&str>,
) -> Option<(RemotePath, Option<String>, Option<String>)> {
    let text = input.text().trim();
    if input.is_composing() || text.is_empty() {
        return None;
    }
    let cwd = cwd.map(str::to_owned);
    if text == clicked.path {
        return Some((clicked.clone(), query.map(str::to_owned), cwd));
    }
    let remote = RemotePath::typed(&clicked.target, clicked.shell, text, cwd.as_deref())?;
    Some((remote, None, cwd))
}

pub(super) fn title(text: String) -> Div {
    div()
        .p(px(8.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
}

pub(super) fn muted(color: u32, text: &'static str) -> Div {
    div().p(px(8.)).text_color(rgb(color)).child(text)
}

pub(super) fn buttons() -> Div {
    div().flex().flex_wrap().gap(px(8.)).p(px(8.))
}
