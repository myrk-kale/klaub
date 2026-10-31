//! The wrapper must keep the harness alive, address its pane by id and paste test text as a
//! bracketed paste **without ever submitting it**. Tests run against a real tmux server on a
//! private socket.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use klaub_core::ports::{AgentChannel, PrepareOutcome};
use klaub_tmux::channel::{PaneCheck, TmuxAgentChannel};
use klaub_tmux::session::{FocusTarget, SessionHealth, focus, health, kill, reap};

use common::{Fixture, HARNESS, contains, tmux_or_skip, wait_until};

const START: &[u8] = b"\x1b[200~";
const END: &[u8] = b"\x1b[201~";

#[test]
fn text_is_pasted_as_a_bracketed_single_line_and_never_submitted() {
    let Some(tmux) = tmux_or_skip() else { return };
    let fx = Fixture::start(&tmux, HARNESS, false);
    fx.wait_ready();

    let channel = TmuxAgentChannel::new(fx.record.clone());
    let outcome = channel
        .prepare_input("Review and resolve the 3 findings in\n.klaub/reviews/rev_1.json.\r\nRead every comment \x1b[31m carefully\x1b]52;c;ZXZpbA==\x07.")
        .unwrap();
    assert_eq!(outcome, PrepareOutcome::Prepared);

    assert!(
        wait_until(Duration::from_secs(5), || contains(&fx.received(), END)),
        "the paste never arrived"
    );
    let bytes = fx.received();
    assert!(
        bytes.starts_with(START),
        "expected a bracketed paste, got {bytes:?}"
    );
    assert!(
        bytes.ends_with(END),
        "nothing may follow the paste (no Enter): {bytes:?}"
    );

    let body = &bytes[START.len()..bytes.len() - END.len()];
    assert!(
        !body.contains(&b'\r') && !body.contains(&b'\n'),
        "line breaks must not reach the harness: {body:?}"
    );
    assert!(
        !body.contains(&0x1b),
        "escape sequences must be stripped: {body:?}"
    );
    assert_eq!(
        String::from_utf8(body.to_vec()).unwrap(),
        "Review and resolve the 3 findings in .klaub/reviews/rev_1.json. Read every comment  carefully."
    );

    // Give a stray Enter time to show up, then prove none did.
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(fx.received(), bytes, "bytes arrived after the paste");
}

#[test]
fn without_bracketed_paste_support_no_line_break_can_submit_anything() {
    let Some(tmux) = tmux_or_skip() else { return };
    let fx = Fixture::start(&tmux, HARNESS, true);
    fx.wait_ready();

    let channel = TmuxAgentChannel::new(fx.record.clone());
    assert_eq!(
        channel
            .prepare_input("first line\nsecond line\nthird")
            .unwrap(),
        PrepareOutcome::Prepared
    );

    assert!(
        wait_until(Duration::from_secs(5), || contains(
            &fx.received(),
            b"third"
        )),
        "the paste never arrived"
    );
    let bytes = fx.received();
    assert!(
        !contains(&bytes, START),
        "the harness did not ask for bracketed paste: {bytes:?}"
    );
    assert!(
        !bytes.contains(&b'\r') && !bytes.contains(&b'\n'),
        "a raw CR or LF would submit input: {bytes:?}"
    );
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        "first line second line third"
    );
}

#[test]
fn existing_input_is_neither_cleared_nor_overwritten() {
    let Some(tmux) = tmux_or_skip() else { return };
    let fx = Fixture::start(&tmux, HARNESS, false);
    fx.wait_ready();

    // The human has started typing.
    fx.client()
        .run([
            "send-keys",
            "-t",
            &fx.record.agent.pane_id,
            "-l",
            "half-typed",
        ])
        .unwrap();
    assert!(wait_until(Duration::from_secs(5), || contains(
        &fx.received(),
        b"half-typed"
    )));

    let channel = TmuxAgentChannel::new(fx.record.clone());
    assert_eq!(
        channel.prepare_input("please review").unwrap(),
        PrepareOutcome::Prepared
    );
    assert!(wait_until(Duration::from_secs(5), || contains(
        &fx.received(),
        END
    )));

    let bytes = fx.received();
    assert!(
        contains(&bytes, b"half-typed\x1b[200~please review\x1b[201~"),
        "{:?}",
        String::from_utf8_lossy(&bytes)
    );
    assert!(
        !bytes.contains(&0x15),
        "Ctrl+U (clear line) must never be sent"
    );
    assert!(!bytes.contains(&b'\r'));
}

#[test]
fn a_replaced_process_in_the_agent_pane_is_refused() {
    let Some(tmux) = tmux_or_skip() else { return };
    let fx = Fixture::start(&tmux, HARNESS, false);
    fx.wait_ready();
    let channel = TmuxAgentChannel::new(fx.record.clone());
    assert!(matches!(channel.verify().unwrap(), PaneCheck::Verified(_)));
    assert!(channel.target().unwrap().is_some());

    // Something else now runs in the very same pane id.
    fx.client()
        .run([
            "respawn-pane",
            "-k",
            "-t",
            &fx.record.agent.pane_id,
            "sleep",
            "600",
        ])
        .unwrap();

    match channel.verify().unwrap() {
        PaneCheck::Rejected(reason) => {
            assert!(reason.contains("replaced"), "{reason}");
        }
        PaneCheck::Verified(_) => panic!("a replaced process must not be trusted"),
    }
    assert!(channel.target().unwrap().is_none());
    match channel.prepare_input("text").unwrap() {
        PrepareOutcome::Unavailable(reason) => {
            assert!(reason.contains("replaced"), "{reason}");
        }
        PrepareOutcome::Prepared => panic!("must not paste into a replaced process"),
    }
}

#[test]
fn a_vanished_agent_pane_is_refused_and_the_session_is_reaped() {
    let Some(tmux) = tmux_or_skip() else { return };
    let fx = Fixture::start(&tmux, HARNESS, false);
    fx.wait_ready();
    let channel = TmuxAgentChannel::new(fx.record.clone());
    assert_eq!(health(&fx.record).unwrap(), SessionHealth::Running);

    fx.client()
        .run(["kill-pane", "-t", &fx.record.agent.pane_id])
        .unwrap();

    assert_eq!(health(&fx.record).unwrap(), SessionHealth::AgentGone);
    match channel.prepare_input("text").unwrap() {
        PrepareOutcome::Unavailable(reason) => {
            assert!(reason.contains("no longer exists"), "{reason}");
        }
        PrepareOutcome::Prepared => panic!("must not paste into a missing pane"),
    }
    assert!(
        reap(&fx.registry, &fx.record).unwrap(),
        "losing the agent ends the session"
    );
    assert!(!fx.client().server_running());
    assert_eq!(fx.registry.list().unwrap(), Vec::new());
}

#[test]
fn the_session_record_exists_before_the_review_window_starts() {
    let Some(tmux) = tmux_or_skip() else { return };
    // The reviewer reads its session record on start. This stand-in fails exactly like the
    // real one does if the record has not been written yet (a regression found in CI).
    let fx = Fixture::start_with_review(&tmux, HARNESS, false, |registry, id| {
        let record = registry.join(format!("{id}.json"));
        [
            "sh",
            "-c",
            "test -f \"$1\" && exec sleep 600",
            "reviewer",
            record.to_str().unwrap(),
        ]
        .map(str::to_owned)
        .to_vec()
    });
    fx.wait_ready();
    std::thread::sleep(Duration::from_millis(500));
    let review = fx.record.review.clone().unwrap();
    let pane = fx
        .client()
        .pane(&review.pane_id)
        .unwrap()
        .expect("the review pane exists");
    assert!(
        !pane.dead,
        "the review window must not start before its session record is on disk"
    );
}

#[test]
fn switching_windows_five_times_keeps_the_harness_and_its_history() {
    let Some(tmux) = tmux_or_skip() else { return };
    let fx = Fixture::start(&tmux, HARNESS, false);
    fx.wait_ready();
    let before = fx.client().pane(&fx.record.agent.pane_id).unwrap().unwrap();

    let mut record = fx.record.clone();
    for _ in 0..5 {
        focus(
            &fx.registry,
            &mut record,
            FocusTarget::Review,
            &fx.review_command,
        )
        .unwrap();
        focus(
            &fx.registry,
            &mut record,
            FocusTarget::Agent,
            &fx.review_command,
        )
        .unwrap();
    }

    let after = fx.client().pane(&fx.record.agent.pane_id).unwrap().unwrap();
    assert_eq!(
        after.pid, before.pid,
        "the harness process must be the same one"
    );
    assert_eq!(after.pane_id, before.pane_id);
    assert!(
        fx.client()
            .capture_pane(&fx.record.agent.pane_id)
            .unwrap()
            .contains("HARNESS-READY"),
        "history was lost"
    );
    assert_eq!(health(&record).unwrap(), SessionHealth::Running);
}

#[test]
fn a_closed_review_window_is_recreated_without_touching_the_agent() {
    let Some(tmux) = tmux_or_skip() else { return };
    let fx = Fixture::start(&tmux, HARNESS, false);
    fx.wait_ready();
    let agent_before = fx.client().pane(&fx.record.agent.pane_id).unwrap().unwrap();
    let old_review = fx.record.review.clone().unwrap();

    fx.client()
        .run(["kill-window", "-t", &old_review.window_id])
        .unwrap();
    assert!(fx.client().pane(&old_review.pane_id).unwrap().is_none());

    let mut record = fx.record.clone();
    focus(
        &fx.registry,
        &mut record,
        FocusTarget::Review,
        &fx.review_command,
    )
    .unwrap();

    let new_review = record.review.clone().unwrap();
    assert_ne!(
        new_review.pane_id, old_review.pane_id,
        "a new review pane was created"
    );
    assert!(fx.client().pane(&new_review.pane_id).unwrap().is_some());
    let agent_after = fx.client().pane(&fx.record.agent.pane_id).unwrap().unwrap();
    assert_eq!(agent_after.pid, agent_before.pid);
    assert_eq!(
        fx.registry.load(&record.session_id).unwrap().review,
        record.review,
        "the registry follows"
    );
}

#[test]
fn ending_one_session_never_touches_another() {
    let Some(tmux) = tmux_or_skip() else { return };
    let first = Fixture::start(&tmux, HARNESS, false);
    let second = Fixture::start(&tmux, HARNESS, false);
    first.wait_ready();
    second.wait_ready();
    assert_ne!(
        first.record.socket, second.record.socket,
        "every session has its own private server"
    );

    kill(&first.registry, &first.record).unwrap();

    assert!(!first.client().server_running());
    assert!(
        second.client().server_running(),
        "the other session must be unaffected"
    );
    assert_eq!(health(&second.record).unwrap(), SessionHealth::Running);
}

#[test]
fn the_private_server_ignores_the_users_tmux_configuration() {
    let Some(tmux) = tmux_or_skip() else { return };
    let fx = Fixture::start(&tmux, HARNESS, false);
    // Klaub's own settings are in force, whatever ~/.tmux.conf says.
    assert_eq!(
        fx.client()
            .run(["show-options", "-gv", "remain-on-exit"])
            .unwrap(),
        "failed"
    );
    assert_eq!(
        fx.client()
            .run(["show-options", "-gv", "escape-time"])
            .unwrap(),
        "10"
    );
    let keys = fx.client().run(["list-keys", "-T", "prefix"]).unwrap();
    assert!(
        keys.contains("session focus --id") || keys.contains("@klaub-focus-review"),
        "{keys}"
    );
}
