#![cfg(unix)]

#[allow(clippy::duplicate_mod, dead_code)]
mod core_terminal_bridge_support;

use forge_runtime_infrastructure::PinnedScheduledReadyNodeReleaseBridge;
use tempfile::tempdir;

use core_terminal_bridge_support::{script_digest, write_script};

#[test]
fn constructor_requires_both_exact_handshakes_before_any_control_input() {
    let directory = tempdir().expect("temporary ready-release Core directory");
    let control_sentinel = directory.path().join("control-input-seen");
    let script = ready_release_script("1", "2", &control_sentinel);
    let path = write_script(directory.path(), "ready-core", &script, true);

    PinnedScheduledReadyNodeReleaseBridge::new(path.clone(), script_digest(&path))
        .expect("both ready-release handshakes");

    assert!(
        !control_sentinel.exists(),
        "constructor sent private control input"
    );
}

#[test]
fn constructor_rejects_reconcile_or_ready_protocol_substitution() {
    for (name, reconcile, ready) in [
        ("wrong-reconcile", "2", "2"),
        ("legacy-ready", "1", "1"),
        ("suffixed-ready", "1", "2x"),
    ] {
        let directory = tempdir().expect("temporary substituted Core directory");
        let sentinel = directory.path().join("control-input-seen");
        let script = ready_release_script(reconcile, ready, &sentinel);
        let path = write_script(directory.path(), name, &script, true);

        assert!(
            PinnedScheduledReadyNodeReleaseBridge::new(path.clone(), script_digest(&path)).is_err(),
            "accepted {name}"
        );
        assert!(!sentinel.exists(), "{name} received private control input");
    }
}

fn ready_release_script(reconcile: &str, ready: &str, sentinel: &std::path::Path) -> String {
    format!(
        "#!/bin/sh\n\
         if [ \"$1\" = \"graph-scheduled-reconcile\" ] && [ \"$2\" = \"--protocol-version\" ]; then printf '%s' '{reconcile}'; exit 0; fi\n\
         if [ \"$1\" = \"graph-scheduled-ready-node-dispatch-authorize\" ] && [ \"$2\" = \"--protocol-version\" ]; then printf '%s' '{ready}'; exit 0; fi\n\
         if [ \"$1\" = \"graph-scheduled-ready-node-dispatch-authorize\" ] && [ \"$2\" = \"--control\" ]; then printf '%s' seen > '{}'; fi\n\
         exit 91\n",
        sentinel.display()
    )
}
