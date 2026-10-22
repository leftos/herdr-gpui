//! A path typed into the folder question is read in the clicked folder's
//! own shell, which its probe confirmed.

use super::*;
use crate::menu::Page;
use folders::folder_window;

#[gpui::test]
fn an_edited_home_path_in_the_folder_question_keeps_powershell(cx: &mut TestAppContext) {
    let host = FakeHost::new();
    let (view, cx) = folder_window(&host, cx);
    click(&view, cx, remote(r"C:\dev\src"), None, None);
    assert_eq!(page(&view, cx), Some(Page::RemoteFolder));
    open(&view, cx, "~/notes");
    assert_eq!(host.asked()[1], remote(r"~\notes"));
}
