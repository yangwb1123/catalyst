use std::io::{self, Write};

use serde::Serialize;

use crate::{
    group_context_output::{GroupContextView, terminal_text, write_group_context},
    runtime_domain::{
        GroupRunRecord, GroupRunSnapshot, GroupRunStatus, PrepareGroupRunDisposition,
    },
};

#[derive(Debug, Serialize)]
pub struct GroupRunSnapshotView {
    pub v: u16,
    pub run: GroupRunRecord,
    pub context: GroupContextView,
}

impl GroupRunSnapshotView {
    pub fn from_snapshot(snapshot: GroupRunSnapshot, include_content: bool) -> Self {
        Self {
            v: snapshot.v,
            run: snapshot.run,
            context: GroupContextView::from_slice(snapshot.context, include_content),
        }
    }
}

pub fn write_group_run_prepared(
    disposition: PrepareGroupRunDisposition,
    snapshot: &GroupRunSnapshotView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "prepared/frozen group run {} — {}",
        terminal_text(&snapshot.run.run_id),
        disposition_label(disposition)
    )?;
    write_execution_boundary(writer)?;
    write_snapshot_metadata(&snapshot.run, writer)?;
    write_group_context(&snapshot.context, writer)
}

pub fn write_group_run(
    snapshot: &GroupRunSnapshotView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "frozen group run {} — {}",
        terminal_text(&snapshot.run.run_id),
        status_label(snapshot.run.status)
    )?;
    write_execution_boundary(writer)?;
    write_snapshot_metadata(&snapshot.run, writer)?;
    write_group_context(&snapshot.context, writer)
}

pub fn write_group_run_list(
    runs: &[GroupRunRecord],
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "prepared/frozen group runs: {} (no model execution)",
        runs.len()
    )?;
    for run in runs {
        writeln!(
            writer,
            "{}\tgroup={}\tstatus={}\tcreated={}\tsnapshot_bytes={}",
            terminal_text(&run.run_id),
            terminal_text(&run.group_id),
            status_label(run.status),
            run.created_at_ms,
            run.snapshot_bytes
        )?;
    }
    Ok(())
}

fn write_execution_boundary(writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(
        writer,
        "model/provider execution: not started (snapshot preparation only)"
    )
}

fn write_snapshot_metadata(run: &GroupRunRecord, writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(
        writer,
        "group={} · context_v={} · snapshot_bytes={}",
        terminal_text(&run.group_id),
        run.context_version,
        run.snapshot_bytes
    )?;
    writeln!(writer, "context sha256 {}", run.context_slice_sha256)?;
    writeln!(writer, "snapshot sha256 {}", run.snapshot_sha256)
}

fn disposition_label(disposition: PrepareGroupRunDisposition) -> &'static str {
    match disposition {
        PrepareGroupRunDisposition::Created => "created",
        PrepareGroupRunDisposition::Replayed => "idempotent replay",
    }
}

fn status_label(status: GroupRunStatus) -> &'static str {
    match status {
        GroupRunStatus::Prepared => "prepared",
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        group_context_output::GroupContextView,
        runtime_domain::{
            Conversation, ConversationScope, GroupContextConversation, GroupContextPayload,
            GroupContextPolicy, GroupContextPrompt, GroupContextProvenance, GroupContextSlice,
            GroupContextStats, GroupRunRecord, GroupRunSnapshot, GroupRunStatus, SessionGroup,
        },
    };

    use super::{GroupRunSnapshotView, write_group_run};

    #[test]
    fn default_snapshot_view_drops_raw_and_per_prompt_private_content() {
        let view = GroupRunSnapshotView::from_snapshot(fixture(), false);
        let encoded = serde_json::to_string(&view).expect("serialize safe view");

        assert!(!encoded.contains("secret excerpt"));
        assert!(!encoded.contains("per-prompt-hash"));
        assert!(!encoded.contains("raw-context-json"));
        assert!(!encoded.contains("idempotency"));
    }

    #[test]
    fn human_snapshot_is_explicitly_prepared_without_execution() {
        let view = GroupRunSnapshotView::from_snapshot(fixture(), false);
        let mut output = Vec::new();
        write_group_run(&view, &mut output).expect("render human output");
        let text = String::from_utf8(output).expect("UTF-8");

        assert!(text.contains("frozen group run group-run-1 — prepared"));
        assert!(text.contains("model/provider execution: not started"));
        assert!(!text.contains("secret excerpt"));
        assert!(!text.contains("per-prompt-hash"));
    }

    #[test]
    fn human_snapshot_escapes_untrusted_identifiers() {
        let mut snapshot = fixture();
        snapshot.run.run_id = "run\n\u{1b}[2J".into();
        snapshot.run.group_id = "group\u{202e}".into();
        snapshot.context.payload.group.id = snapshot.run.group_id.clone();
        let view = GroupRunSnapshotView::from_snapshot(snapshot, false);
        let mut output = Vec::new();

        write_group_run(&view, &mut output).expect("render human output");
        let text = String::from_utf8(output).expect("human output is UTF-8");
        assert!(text.contains("run\\n\\x1b[2J"));
        assert!(text.contains("group\\u{202e}"));
        assert!(!text.contains('\u{1b}'));
        assert!(!text.contains('\u{202e}'));
    }

    fn fixture() -> GroupRunSnapshot {
        GroupRunSnapshot {
            v: 1,
            run: run_record(),
            context: context(),
            context_json: "raw-context-json idempotency".into(),
        }
    }

    fn run_record() -> GroupRunRecord {
        GroupRunRecord {
            v: 1,
            run_id: "group-run-1".into(),
            group_id: "group-1".into(),
            status: GroupRunStatus::Prepared,
            context_version: 1,
            context_slice_sha256: "context-hash".into(),
            snapshot_sha256: "snapshot-hash".into(),
            snapshot_bytes: 123,
            created_at_ms: 42,
        }
    }

    fn context() -> GroupContextSlice {
        GroupContextSlice {
            v: 1,
            payload: GroupContextPayload {
                policy: GroupContextPolicy::default(),
                group: SessionGroup {
                    id: "group-1".into(),
                    name: "Identity".into(),
                    created_at_ms: 1,
                },
                members: vec![],
                conversations: vec![conversation()],
                stats: GroupContextStats {
                    conversation_count: 1,
                    prompt_count: 1,
                    content_bytes: 14,
                    ..GroupContextStats::default()
                },
            },
            slice_sha256: "context-hash".into(),
        }
    }

    fn conversation() -> GroupContextConversation {
        GroupContextConversation {
            conversation: Conversation {
                id: "session-1".into(),
                scope: ConversationScope::Group("group-1".into()),
                title: "Discussion".into(),
                created_at_ms: 2,
                updated_at_ms: 3,
            },
            provenance: GroupContextProvenance::Group {
                group_id: "group-1".into(),
            },
            prompts: vec![GroupContextPrompt {
                id: "prompt-1".into(),
                role: "user".into(),
                created_at_ms: 3,
                excerpt: "secret excerpt".into(),
                original_bytes: 14,
                content_sha256: "per-prompt-hash".into(),
                truncated: false,
            }],
            omitted_prompt_count: 0,
        }
    }

    #[test]
    fn include_content_uses_the_existing_context_view_semantics() {
        let GroupContextView { payload, .. } =
            GroupRunSnapshotView::from_snapshot(fixture(), true).context;
        assert_eq!(
            payload.conversations[0].prompts[0].excerpt.as_deref(),
            Some("secret excerpt")
        );
    }
}
