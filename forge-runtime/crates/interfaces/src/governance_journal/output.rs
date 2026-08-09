use std::io::{self, Write};

use serde::Serialize;

use crate::group_context_output::terminal_text;
use crate::runtime_domain::{
    AppendGovernanceRecordBatchResult, GovernanceRecordAppendDisposition,
    GovernanceRecordInspection, GovernanceRecordKind, GovernanceStructuralHead,
};

const PUBLIC_API_VERSION: &str = "forgeos.governance-journal/v1";

#[derive(Debug)]
pub enum GovernanceJournalOutput {
    Append(AppendReceiptView),
    Inspection(InspectionView),
    List(InspectionListView),
    Head(StructuralHeadView),
}

#[derive(Debug, Serialize)]
pub struct AppendReceiptView {
    pub api_version: &'static str,
    pub kind: &'static str,
    pub batch_id: String,
    pub request_sha256: String,
    pub record_set_sha256: String,
    pub disposition: &'static str,
    pub record_count: usize,
    pub record_ids: Vec<String>,
    pub appended_at_unix_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct InspectionView {
    pub api_version: &'static str,
    pub kind: &'static str,
    pub batch_id: String,
    pub batch_ordinal: usize,
    pub record_id: String,
    pub record_kind: GovernanceRecordKind,
    pub aggregate_id: String,
    pub sequence: i64,
    pub canonical_sha256: String,
    pub canonical_record_bytes: usize,
    pub created_at_unix_ms: i64,
    pub appended_at_unix_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_record_json: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InspectionListView {
    pub api_version: &'static str,
    pub kind: &'static str,
    pub records: Vec<InspectionView>,
}

#[derive(Debug, Serialize)]
pub struct StructuralHeadView {
    pub api_version: &'static str,
    pub kind: &'static str,
    pub interpretation: &'static str,
    pub record_kind: GovernanceRecordKind,
    pub aggregate_id: String,
    pub record_id: String,
    pub sequence: i64,
    pub canonical_sha256: String,
    pub updated_at_unix_ms: u64,
}

impl From<AppendGovernanceRecordBatchResult> for AppendReceiptView {
    fn from(result: AppendGovernanceRecordBatchResult) -> Self {
        let receipt = result.receipt;
        Self {
            api_version: PUBLIC_API_VERSION,
            kind: "GovernanceRecordAppendReceipt",
            batch_id: receipt.batch_id,
            request_sha256: receipt.request_sha256,
            record_set_sha256: receipt.record_set_sha256,
            disposition: disposition_name(result.disposition),
            record_count: receipt.record_count,
            record_ids: receipt.record_ids,
            appended_at_unix_ms: receipt.appended_at_ms,
        }
    }
}

impl From<GovernanceRecordInspection> for InspectionView {
    fn from(inspection: GovernanceRecordInspection) -> Self {
        let metadata = inspection.metadata;
        Self {
            api_version: PUBLIC_API_VERSION,
            kind: "GovernanceRecordInspection",
            batch_id: metadata.batch_id,
            batch_ordinal: metadata.batch_ordinal,
            record_id: metadata.record_id,
            record_kind: metadata.record_kind,
            aggregate_id: metadata.aggregate_id,
            sequence: metadata.sequence,
            canonical_sha256: metadata.canonical_sha256,
            canonical_record_bytes: metadata.canonical_record_bytes,
            created_at_unix_ms: metadata.created_at_unix_ms,
            appended_at_unix_ms: metadata.appended_at_ms,
            canonical_record_json: inspection.canonical_record_json,
        }
    }
}

impl From<Vec<GovernanceRecordInspection>> for InspectionListView {
    fn from(inspections: Vec<GovernanceRecordInspection>) -> Self {
        Self {
            api_version: PUBLIC_API_VERSION,
            kind: "GovernanceRecordInspectionList",
            records: inspections.into_iter().map(InspectionView::from).collect(),
        }
    }
}

impl From<GovernanceStructuralHead> for StructuralHeadView {
    fn from(head: GovernanceStructuralHead) -> Self {
        Self {
            api_version: PUBLIC_API_VERSION,
            kind: "GovernanceStructuralHead",
            interpretation: "structural_sequence_only",
            record_kind: head.record_kind,
            aggregate_id: head.aggregate_id,
            record_id: head.record_id,
            sequence: head.sequence,
            canonical_sha256: head.canonical_sha256,
            updated_at_unix_ms: head.updated_at_ms,
        }
    }
}

pub fn write_output(
    output: &GovernanceJournalOutput,
    json: bool,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    if json {
        write_json(output, writer)?;
        writeln!(writer)?;
        return Ok(());
    }
    write_human(output, writer)
}

fn write_json(output: &GovernanceJournalOutput, writer: &mut impl Write) -> Result<(), io::Error> {
    match output {
        GovernanceJournalOutput::Append(value) => serde_json::to_writer_pretty(writer, value)?,
        GovernanceJournalOutput::Inspection(value) => serde_json::to_writer_pretty(writer, value)?,
        GovernanceJournalOutput::List(value) => serde_json::to_writer_pretty(writer, value)?,
        GovernanceJournalOutput::Head(value) => serde_json::to_writer_pretty(writer, value)?,
    }
    Ok(())
}

fn write_human(output: &GovernanceJournalOutput, writer: &mut impl Write) -> Result<(), io::Error> {
    match output {
        GovernanceJournalOutput::Append(value) => writeln!(
            writer,
            "{} {} records in {}",
            value.disposition, value.record_count, value.batch_id
        ),
        GovernanceJournalOutput::Inspection(value) => write_inspection(value, writer),
        GovernanceJournalOutput::List(value) => {
            for value in &value.records {
                write_inspection(value, writer)?;
            }
            Ok(())
        }
        GovernanceJournalOutput::Head(value) => writeln!(
            writer,
            "{} {} sequence {} -> {} ({})",
            value.record_kind.as_str(),
            value.aggregate_id,
            value.sequence,
            value.record_id,
            value.interpretation
        ),
    }
}

fn write_inspection(value: &InspectionView, writer: &mut impl Write) -> Result<(), io::Error> {
    writeln!(
        writer,
        "{} {} {} sequence {} digest {}",
        value.record_kind.as_str(),
        value.record_id,
        value.aggregate_id,
        value.sequence,
        value.canonical_sha256
    )?;
    if let Some(record) = &value.canonical_record_json {
        writeln!(writer, "{}", terminal_text(record))?;
    }
    Ok(())
}

const fn disposition_name(disposition: GovernanceRecordAppendDisposition) -> &'static str {
    match disposition {
        GovernanceRecordAppendDisposition::Stored => "stored",
        GovernanceRecordAppendDisposition::ExactReplay => "exact_replay",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_human_reveal_escapes_terminal_controls() {
        let output = GovernanceJournalOutput::Inspection(InspectionView {
            api_version: PUBLIC_API_VERSION,
            kind: "GovernanceRecordInspection",
            batch_id: "governance-record-batch-test".into(),
            batch_ordinal: 0,
            record_id: "evr-test".into(),
            record_kind: GovernanceRecordKind::EvidenceRecord,
            aggregate_id: "evidence-test".into(),
            sequence: 1,
            canonical_sha256: "0".repeat(64),
            canonical_record_bytes: 1,
            created_at_unix_ms: 1,
            appended_at_unix_ms: 1,
            canonical_record_json: Some("{\"text\":\"\u{009b}2J\"}".into()),
        });
        let mut rendered = Vec::new();
        write_output(&output, false, &mut rendered).expect("write human output");
        let rendered = String::from_utf8(rendered).expect("UTF-8 output");
        assert!(!rendered.contains('\u{009b}'));
        assert!(rendered.contains("\\u{9b}2J"), "{rendered:?}");
    }
}
