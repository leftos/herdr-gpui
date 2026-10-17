//! A drop held across a projection gap onto a WSL pane.

use super::*;
use crate::window::pending_input::DropAt;

#[gpui::test]
fn a_held_drop_maps_its_paths_and_leads_the_keys_held_behind_it(cx: &mut TestAppContext) {
    let (fixture, cx) = cx.add_window_view(|window, cx| Fixture {
        view: cx.new(|cx| fixture_window(window, cx)),
    });
    let view = fixture.read_with(cx, |fixture, _| fixture.view.clone());
    let mut peer = MockPeer::new();
    view.update(cx, |view, cx| {
        peer.prepare(view, cx);
        // An invalid name: should the real mapping ever run, it is refused
        // before `wsl.exe` starts, and its late answer is for a finished drop.
        view.endpoints[0].connection.target = ConnectTarget::Wsl {
            distro: "a;b".into(),
            session: "default".into(),
        };
        assert!(view.flush_pending_input(cx));
        open_gap(view);
        let at = DropAt::Pane(InputTarget::Pane(PANE.into()));
        assert!(view.hold_drop(at, vec![PathBuf::from("C:/Users/me/a.txt")], cx));
        view.send(ClientPaneInputEvent::TextCommit("x".into()), cx);
        close_gap(view);
        // The frame returns: the drop starts mapping, and the key waits.
        assert!(view.flush_pending_input(cx));
        assert!(view.wsl_paste.is_some(), "the drop maps its paths");
        assert!(view.file_transfer.is_none());
        assert!(view.local_error.is_none(), "{:?}", view.local_error);
        assert!(!view.flush_pending_input(cx));
        finish(view, &["/mnt/c/Users/me/a.txt"], cx);
        assert!(view.wsl_paste.is_none());
        assert!(view.local_error.is_none(), "{:?}", view.local_error);
        assert!(view.file_transfer.is_none());
    });
    assert_eq!(
        peer.receive(),
        pane_input(vec![ClientPaneInputEvent::Paste(
            "/mnt/c/Users/me/a.txt".into()
        )])
    );
    assert_eq!(
        peer.receive(),
        pane_input(vec![ClientPaneInputEvent::TextCommit("x".into())])
    );
    view.update(cx, |view, _| sentinel(view, &mut peer));
}
