use crate::runtime_domain::execution::attempt::AttemptRequest;
use sha2::{Digest, Sha256};

use super::{
    AttemptJournalError, MAX_REQUEST_BYTES,
    codec_model::{FORMAT, StoredRequest},
};

const DIGEST_DOMAIN: &[u8] = b"forge.runtime.attempt-request.storage.v1\0";

pub(super) struct RequestRecord {
    pub json: String,
    pub sha256: String,
}

pub(super) fn encode(request: &AttemptRequest) -> Result<RequestRecord, AttemptJournalError> {
    let mut value = serde_json::to_value(StoredRequest::from_request(request))
        .map_err(|_| AttemptJournalError::Invalid)?;
    value.sort_all_objects();
    let json = serde_json::to_string(&value).map_err(|_| AttemptJournalError::Invalid)?;
    if json.len() > MAX_REQUEST_BYTES {
        return Err(AttemptJournalError::Invalid);
    }
    let mut hash = Sha256::new();
    hash.update(DIGEST_DOMAIN);
    hash.update(json.as_bytes());
    Ok(RequestRecord {
        json,
        sha256: format!("{:x}", hash.finalize()),
    })
}

pub(super) fn decode(json: &str) -> Result<AttemptRequest, AttemptJournalError> {
    if json.is_empty() || json.len() > MAX_REQUEST_BYTES {
        return Err(AttemptJournalError::Corrupt);
    }
    let record: StoredRequest =
        serde_json::from_str(json).map_err(|_| AttemptJournalError::Corrupt)?;
    if record.format != FORMAT || record.version != 1 {
        return Err(AttemptJournalError::Corrupt);
    }
    let request = AttemptRequest::try_from_input(&record.into_input())
        .map_err(|_| AttemptJournalError::Corrupt)?;
    if encode(&request)
        .map_err(|_| AttemptJournalError::Corrupt)?
        .json
        != json
    {
        return Err(AttemptJournalError::Corrupt);
    }
    Ok(request)
}
