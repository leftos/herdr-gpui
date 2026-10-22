//! A raw-joined click on a folder path: the which-path question comes
//! before any probe or reopened copy, and a chosen path that is a folder
//! then asks the folder question.

use super::*;
use crate::menu::Page;
use folders::{folder_window, zip_of};
use gpui::Modifiers;

const FIRST: &str = "/home/me/sr";
const JOINED: &str = "/home/me/src";

/// A click on [`JOINED`], whose first row alone reads [`FIRST`].
fn raw_click(view: &Entity<HerdrWindow>, cx: &mut VisualTestContext) {
    click(view, cx, remote(JOINED), Some(remote(FIRST)), None);
}

#[gpui::test]
fn a_raw_joined_click_on_a_cached_folder_asks_before_probing(cx: &mut TestAppContext) {
    let host = FakeHost::new();
    let (view, cx) = folder_window(&host, cx);
    host.serve_archive(&zip_of(&[("src/notes.md", b"notes")]));
    click(&view, cx, remote(JOINED), None, None);
    assert_eq!(page(&view, cx), Some(Page::RemoteFolder));
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    let copy = cx.debug_bounds("remote-folder-copy").unwrap().center();
    cx.simulate_click(copy, Modifiers::default());
    cx.run_until_parked();
    assert!(view.read_with(cx, |view, _| {
        view.remote_files.folder("me@devbox", JOINED).is_some()
    }));
    let (asked, probes) = (host.asked().len(), host.probes().len());
    raw_click(&view, cx);
    assert_eq!(page(&view, cx), Some(Page::RemoteJoin));
    assert_eq!(host.probes().len(), probes, "nothing is probed");
    assert_eq!(host.asked().len(), asked, "nothing is fetched");
    assert_eq!(host.archives().len(), 1);
}

#[gpui::test]
fn open_of_a_joined_path_that_is_a_folder_asks_the_folder_question(cx: &mut TestAppContext) {
    let host = FakeHost::new();
    let (view, cx) = folder_window(&host, cx);
    raw_click(&view, cx);
    assert_eq!(page(&view, cx), Some(Page::RemoteJoin));
    assert!(host.probes().is_empty());
    cx.simulate_keystrokes("enter");
    poll(&view, cx);
    assert_eq!(host.asked(), vec![remote(JOINED)]);
    assert_eq!(page(&view, cx), Some(Page::RemoteFolder));
    let shown = view.read_with(cx, |view, _| view.remote_folder_text());
    assert_eq!(shown.expect("the folder asks")[1], JOINED);
    assert!(host.archives().is_empty(), "nothing is copied");
}
