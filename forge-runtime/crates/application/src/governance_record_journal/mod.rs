mod service;
mod validation;

pub use service::{
    AppendGovernanceRecordBatchInput, GovernanceRecordJournalService,
    GovernanceRecordJournalServiceError,
};

#[cfg(test)]
mod tests;
