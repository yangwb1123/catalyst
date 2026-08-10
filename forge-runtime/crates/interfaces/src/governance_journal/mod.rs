use std::{
    error::Error,
    fs::{self, File},
    io::{self, Read},
    path::Path,
    sync::Arc,
};

use forge_runtime_infrastructure::SqliteHubStore;

mod output;
pub use output::{GovernanceJournalOutput, write_output};

use crate::{
    args::{Args, GovernanceCommand, GovernanceJournalCommand},
    runtime_application::{AppendGovernanceRecordBatchInput, GovernanceRecordJournalService},
    runtime_domain::{GovernanceRecordListFilter, governance_contract::MAX_RECORD_SET_BYTES},
    state_path::{hub_database_path, unix_time_millis},
};
use output::{AppendReceiptView, InspectionListView, InspectionView, StructuralHeadView};

pub fn execute(
    args: &Args,
    command: &GovernanceCommand,
) -> Result<GovernanceJournalOutput, Box<dyn Error>> {
    let GovernanceCommand::Journal(command) = command;
    match command {
        GovernanceJournalCommand::Append { file } => append(args, file),
        GovernanceJournalCommand::Show {
            record_id,
            include_record,
        } => show(args, record_id, *include_record),
        GovernanceJournalCommand::List {
            record_kind,
            aggregate_id,
            limit,
            include_record,
        } => list(
            args,
            &GovernanceRecordListFilter {
                record_kind: *record_kind,
                aggregate_id: aggregate_id.clone(),
                limit: *limit,
                include_record: *include_record,
            },
        ),
        GovernanceJournalCommand::Head {
            record_kind,
            aggregate_id,
        } => head(args, *record_kind, aggregate_id),
    }
}

fn append(args: &Args, file: &Path) -> Result<GovernanceJournalOutput, Box<dyn Error>> {
    let idempotency_key = args
        .idempotency_key
        .clone()
        .ok_or("governance journal append requires an explicit idempotency key")?;
    GovernanceRecordJournalService::preflight_append_key(&idempotency_key)?;
    let canonical_record_set_json = read_record_set(file)?;
    let request =
        GovernanceRecordJournalService::prepare_append_request(AppendGovernanceRecordBatchInput {
            canonical_record_set_json,
            idempotency_key,
            appended_at_ms: unix_time_millis(),
        })?;
    let store = Arc::new(SqliteHubStore::open(hub_database_path(
        args.state_dir.as_deref(),
    )?)?);
    let result = GovernanceRecordJournalService::new(store).append_prepared(&request)?;
    Ok(GovernanceJournalOutput::Append(AppendReceiptView::from(
        result,
    )))
}

fn show(
    args: &Args,
    record_id: &str,
    include_record: bool,
) -> Result<GovernanceJournalOutput, Box<dyn Error>> {
    GovernanceRecordJournalService::preflight_inspect(record_id)?;
    let service = read_service(args)?;
    let inspection = service.inspect(record_id, include_record)?;
    Ok(GovernanceJournalOutput::Inspection(InspectionView::from(
        inspection,
    )))
}

fn list(
    args: &Args,
    filter: &GovernanceRecordListFilter,
) -> Result<GovernanceJournalOutput, Box<dyn Error>> {
    GovernanceRecordJournalService::preflight_list(filter)?;
    let values = read_service(args)?.list(filter)?;
    Ok(GovernanceJournalOutput::List(InspectionListView::from(
        values,
    )))
}

fn head(
    args: &Args,
    record_kind: crate::runtime_domain::GovernanceRecordKind,
    aggregate_id: &str,
) -> Result<GovernanceJournalOutput, Box<dyn Error>> {
    GovernanceRecordJournalService::preflight_head(aggregate_id)?;
    let value = read_service(args)?.inspect_structural_head(record_kind, aggregate_id)?;
    Ok(GovernanceJournalOutput::Head(StructuralHeadView::from(
        value,
    )))
}

fn read_service(args: &Args) -> Result<GovernanceRecordJournalService, Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open_existing_current_read_only(database)?);
    Ok(GovernanceRecordJournalService::new(store))
}

fn read_record_set(path: &Path) -> Result<String, Box<dyn Error>> {
    let bytes = if path == Path::new("-") {
        read_bounded(io::stdin().lock())?
    } else {
        read_bounded(open_regular_file(path)?)?
    };
    String::from_utf8(bytes).map_err(Into::into)
}

fn open_regular_file(path: &Path) -> Result<File, io::Error> {
    let before = fs::symlink_metadata(path)?;
    ensure_regular_not_symlink(&before)?;
    let file = File::open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() {
        return Err(invalid_file_type());
    }
    let after = fs::symlink_metadata(path)?;
    ensure_regular_not_symlink(&after)?;
    #[cfg(unix)]
    ensure_same_file(&opened, &after)?;
    Ok(file)
}

fn ensure_regular_not_symlink(metadata: &fs::Metadata) -> Result<(), io::Error> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid_file_type());
    }
    Ok(())
}

fn invalid_file_type() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "governance record-set path must be a regular non-symlink file",
    )
}

#[cfg(unix)]
fn ensure_same_file(opened: &fs::Metadata, current: &fs::Metadata) -> Result<(), io::Error> {
    use std::os::unix::fs::MetadataExt;

    if opened.dev() != current.dev() || opened.ino() != current.ino() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance record-set path changed while opening",
        ));
    }
    Ok(())
}

fn read_bounded(reader: impl Read) -> Result<Vec<u8>, io::Error> {
    let limit = u64::try_from(MAX_RECORD_SET_BYTES + 1).expect("record-set limit fits u64");
    let mut bytes = Vec::new();
    reader.take(limit).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_RECORD_SET_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("governance record set exceeds {MAX_RECORD_SET_BYTES} bytes"),
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::read_bounded;
    use crate::runtime_domain::governance_contract::MAX_RECORD_SET_BYTES;

    #[test]
    fn bounded_reader_accepts_the_limit_and_rejects_one_more_byte() {
        assert_eq!(
            read_bounded(&vec![b'x'; MAX_RECORD_SET_BYTES][..])
                .expect("exact limit")
                .len(),
            MAX_RECORD_SET_BYTES
        );
        assert!(read_bounded(&vec![b'x'; MAX_RECORD_SET_BYTES + 1][..]).is_err());
    }
}
