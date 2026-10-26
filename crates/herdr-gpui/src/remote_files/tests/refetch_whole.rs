//! A re-fetch whose old folder copy another application holds open: the
//! copy stays whole rather than half removed.

use super::folders::{
    PATH, answer, changed, click, click_and_copy, flash_text, folder_window, held_beside, probed,
    zip_of,
};
use super::*;
use std::io::Read;
use std::os::windows::fs::OpenOptionsExt;

#[gpui::test]
fn a_refetch_over_a_copy_in_use_keeps_the_whole_copy(cx: &mut TestAppContext) {
    let host = FakeHost::new();
    let (view, cx) = folder_window(&host, cx);
    host.serve_archive(&zip_of(&[("src/a.txt", b"old a"), ("src/b.txt", b"old b")]));
    view.update(cx, |_, _| host.answer_probes(&[probed, changed]));
    click_and_copy(&view, cx, PATH);
    let (local, _) = view
        .read_with(cx, |view, _| view.remote_files.folder("me@devbox", PATH))
        .expect("the folder is copied");
    // No sharing at all, as an editor that locks its file holds it. The held
    // file sorts last: `remove_dir_all` stops at the first error in name
    // order, so only a held last entry shows the files removed before it.
    let mut held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(local.join("b.txt"))
        .unwrap();
    host.serve_archive(&zip_of(&[("src/a.txt", b"new a"), ("src/b.txt", b"new b")]));
    click(&view, cx, PATH);
    answer(&view, cx, "remote-folder-refetch");
    assert_eq!(host.archives().len(), 2, "Re-fetch transfers");
    let text = flash_text(&view, cx).expect("the folder in use is reported");
    assert!(
        text.contains("it may be open in another application"),
        "{text}"
    );
    assert_eq!(
        std::fs::read(local.join("a.txt")).ok(),
        Some(b"old a".to_vec()),
        "the file not held stays"
    );
    assert!(local.join("b.txt").exists(), "the held file stays");
    let mut b = Vec::new();
    held.read_to_end(&mut b).unwrap();
    assert_eq!(b, b"old b");
    assert_eq!(
        held_beside(&local),
        [local.clone()].into(),
        "no transient folder is left beside the copy"
    );
    drop(held);
}
