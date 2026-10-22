//! A click whose SSH path a raw wrap ran onto the next row, where the first
//! row alone reads as another path: the question asked before anything is
//! fetched, and what its answers open.

use super::*;
use crate::menu::Page;
use gpui::Modifiers;

const CWD: &str = "/home/me/src";
const FIRST: &str = "/home/me/src/notes.md";
const JOINED: &str = "/home/me/src/notes.mddone";

/// A click on [`JOINED`], whose first row alone reads [`FIRST`].
fn raw_click(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext) {
    click(view, cx, posix(JOINED), Some(posix(FIRST)), Some(CWD));
}

/// The path the raw-join question's field holds.
fn join_field(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext) -> Option<String> {
    view.read_with(cx, |view, cx| view.remote_join_text(cx))
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
}

/// Clicks the button whose element is `id`.
fn press(cx: &mut VisualTestContext, id: &'static str) {
    draw(cx);
    let button = cx.debug_bounds(id).unwrap().center();
    cx.simulate_click(button, Modifiers::default());
    cx.run_until_parked();
}

#[gpui::test]
fn a_raw_joined_click_asks_which_path_before_fetching(cx: &mut TestAppContext) {
    let host = FakeHost::new();
    let (view, cx) = window(&host, cx);
    raw_click(&view, cx);
    assert_eq!(page(&view, cx), Some(Page::RemoteJoin));
    assert_eq!(Page::RemoteJoin.accessible_name(), "Open remote file");
    assert_eq!(join_field(&view, cx).as_deref(), Some(JOINED));
    assert_eq!(
        view.read_with(cx, |view, _| view.remote_join_words()),
        Some((
            "Open from me@devbox?".to_owned(),
            "This path ran onto a second line, which may be other output."
        ))
    );
    draw(cx);
    for id in [
        "remote-join-open",
        "remote-join-first-line",
        "remote-join-cancel",
    ] {
        assert!(cx.debug_bounds(id).is_some(), "{id}");
    }
    assert!(host.asked().is_empty());
    assert!(cx.opened_url().is_none());
}

#[gpui::test]
fn open_fetches_the_joined_path_with_the_click_s_query_and_line(cx: &mut TestAppContext) {
    let host = FakeHost::new();
    let (view, cx) = window(&host, cx);
    let opened = Arc::new(Mutex::new(Vec::new()));
    let record = opened.clone();
    let joined = RemotePath {
        line: Some(3),
        ..posix("/srv/www/site/index.html")
    };
    let clicked = joined.clone();
    view.update(cx, |view, cx| {
        view.default_browser = Arc::new(move |url: &url::Url| {
            record.lock().unwrap().push(url.to_string());
            Ok(())
        });
        let first = Some(posix("/srv/www/site"));
        view.open_remote_file(
            clicked,
            Some("?a=1#top".into()),
            first,
            Some(CWD.into()),
            cx,
        )
    });
    poll(&view, cx);
    assert!(host.asked().is_empty());
    cx.simulate_keystrokes("enter");
    poll(&view, cx);
    assert_eq!(host.asked(), vec![joined]);
    assert_eq!(page(&view, cx), None);
    let opened = opened.lock().unwrap().clone();
    assert_eq!(opened.len(), 1);
    assert!(opened[0].ends_with("index.html?a=1#top"), "{opened:?}");
}

#[gpui::test]
fn first_line_only_offers_the_first_row_s_part_and_open_fetches_it(cx: &mut TestAppContext) {
    let host = FakeHost::new();
    let (view, cx) = window(&host, cx);
    raw_click(&view, cx);
    press(cx, "remote-join-first-line");
    assert_eq!(join_field(&view, cx).as_deref(), Some(FIRST));
    assert_eq!(page(&view, cx), Some(Page::RemoteJoin));
    assert!(host.asked().is_empty());
    cx.simulate_keystrokes("enter");
    poll(&view, cx);
    assert_eq!(host.asked(), vec![posix(FIRST)]);
    assert_eq!(page(&view, cx), None);
}

#[gpui::test]
fn typing_after_first_line_only_replaces_the_offered_path(cx: &mut TestAppContext) {
    let host = FakeHost::new();
    let (view, cx) = window(&host, cx);
    raw_click(&view, cx);
    press(cx, "remote-join-first-line");
    open(&view, cx, "/srv/typed.md");
    assert_eq!(host.asked(), vec![posix("/srv/typed.md")]);
}

#[gpui::test]
fn an_edited_home_path_keeps_the_windows_host_s_shell(cx: &mut TestAppContext) {
    let host = FakeHost::new();
    let (view, cx) = window(&host, cx);
    let (joined, first) = (r"C:\dev\notes.mddone", r"C:\dev\notes.md");
    click(&view, cx, remote(joined), Some(remote(first)), None);
    assert_eq!(page(&view, cx), Some(Page::RemoteJoin));
    open(&view, cx, "~/notes.md");
    assert_eq!(host.asked(), vec![remote(r"~\notes.md")]);
}

#[gpui::test]
fn cancel_fetches_nothing_and_a_later_click_asks_again(cx: &mut TestAppContext) {
    let host = FakeHost::new();
    let (view, cx) = window(&host, cx);
    raw_click(&view, cx);
    press(cx, "remote-join-cancel");
    poll(&view, cx);
    assert_eq!(page(&view, cx), None);
    assert_eq!(join_field(&view, cx), None);
    raw_click(&view, cx);
    assert_eq!(page(&view, cx), Some(Page::RemoteJoin));
    assert!(host.asked().is_empty());
}

#[gpui::test]
fn escape_fetches_nothing_and_a_later_click_asks_again(cx: &mut TestAppContext) {
    let host = FakeHost::new();
    let (view, cx) = window(&host, cx);
    raw_click(&view, cx);
    cx.simulate_keystrokes("escape");
    poll(&view, cx);
    assert_eq!(page(&view, cx), None);
    raw_click(&view, cx);
    assert_eq!(page(&view, cx), Some(Page::RemoteJoin));
    assert!(host.asked().is_empty());
}

#[gpui::test]
fn a_second_click_while_the_question_is_open_asks_nothing_more(cx: &mut TestAppContext) {
    let host = FakeHost::new();
    let (view, cx) = window(&host, cx);
    raw_click(&view, cx);
    raw_click(&view, cx);
    cx.simulate_keystrokes("escape");
    poll(&view, cx);
    assert_eq!(page(&view, cx), None);
    assert!(host.asked().is_empty());
}

#[gpui::test]
fn clicks_while_another_page_is_open_wait_as_one_question(cx: &mut TestAppContext) {
    let host = FakeHost::new();
    let (view, cx) = window(&host, cx);
    view.update_in(cx, |view, window, cx| assert!(view.open_menu(window, cx)));
    raw_click(&view, cx);
    raw_click(&view, cx);
    assert_eq!(page(&view, cx), Some(Page::Menu));
    view.update_in(cx, |view, window, cx| view.dismiss_menu(window, cx));
    poll(&view, cx);
    assert_eq!(page(&view, cx), Some(Page::RemoteJoin));
    cx.simulate_keystrokes("escape");
    poll(&view, cx);
    assert_eq!(page(&view, cx), None);
    assert!(host.asked().is_empty());
}

/// Sets the state of the copy of [`JOINED`] or another fetched path.
fn set_state(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext, path: &str, state: SyncState) {
    view.update(cx, |view, _| {
        let key = ("me@devbox".to_owned(), path.to_owned());
        view.remote_files.copies.get_mut(&key).unwrap().state = state;
    });
}

#[gpui::test]
fn the_question_waits_behind_an_upload_and_opens_before_a_missing_file(cx: &mut TestAppContext) {
    const SAVED: &str = "/home/me/saved.md";
    let host = FakeHost::new();
    let (view, cx) = window(&host, cx);
    click(&view, cx, posix(SAVED), None, Some(CWD));
    let sha256 = Sha256::digest(b"# Saved\n").into();
    set_state(&view, cx, SAVED, SyncState::Asking { sha256 });
    view.update(cx, |view, _| {
        view.remote_files.missing.push_back(Missing {
            remote: posix("/home/me/gone.md"),
            query: None,
            cwd: None,
        });
    });
    raw_click(&view, cx);
    assert_eq!(page(&view, cx), Some(Page::RemoteUpload));
    cx.simulate_keystrokes("escape");
    poll(&view, cx);
    assert_eq!(page(&view, cx), Some(Page::RemoteJoin));
    cx.simulate_keystrokes("escape");
    poll(&view, cx);
    assert_eq!(page(&view, cx), Some(Page::RemoteMissing));
    assert_eq!(host.asked(), vec![posix(SAVED)]);
}

#[gpui::test]
fn open_lets_a_later_identical_click_ask_again(cx: &mut TestAppContext) {
    let host = FakeHost::new();
    let (view, cx) = window(&host, cx);
    raw_click(&view, cx);
    cx.simulate_keystrokes("enter");
    poll(&view, cx);
    assert_eq!(host.asked(), vec![posix(JOINED)]);
    assert_eq!(page(&view, cx), None);
    raw_click(&view, cx);
    assert_eq!(page(&view, cx), Some(Page::RemoteJoin));
    assert_eq!(host.asked().len(), 1);
}

#[gpui::test]
fn a_raw_joined_click_on_a_copy_with_unsynced_edits_asks_before_reopening_it(
    cx: &mut TestAppContext,
) {
    let host = FakeHost::new();
    let (view, cx) = window(&host, cx);
    click(&view, cx, posix(JOINED), None, Some(CWD));
    let sha256 = Sha256::digest(b"# Notes, edited\n").into();
    set_state(&view, cx, JOINED, SyncState::Edited { sha256 });
    raw_click(&view, cx);
    assert_eq!(page(&view, cx), Some(Page::RemoteJoin));
    assert_eq!(host.asked().len(), 1);
}

#[gpui::test]
fn a_raw_joined_click_on_a_fetched_copy_asks_before_opening_it(cx: &mut TestAppContext) {
    let host = FakeHost::new();
    let (view, cx) = window(&host, cx);
    click(&view, cx, posix(JOINED), None, Some(CWD));
    assert_eq!(host.asked(), vec![posix(JOINED)]);
    assert!(view.read_with(cx, |view, _| {
        view.remote_files.copy("me@devbox", JOINED).is_some()
    }));
    raw_click(&view, cx);
    assert_eq!(page(&view, cx), Some(Page::RemoteJoin));
    assert_eq!(host.asked().len(), 1);
}

#[gpui::test]
fn open_of_a_joined_path_the_host_lacks_asks_for_a_path_holding_it(cx: &mut TestAppContext) {
    let host = FakeHost::new();
    let (view, cx) = on_empty_host(&host, cx);
    raw_click(&view, cx);
    cx.simulate_keystrokes("enter");
    poll(&view, cx);
    assert_eq!(host.asked(), vec![posix(JOINED)]);
    assert_eq!(page(&view, cx), Some(Page::RemoteMissing));
    assert_eq!(field(&view, cx).as_deref(), Some(JOINED));
}
