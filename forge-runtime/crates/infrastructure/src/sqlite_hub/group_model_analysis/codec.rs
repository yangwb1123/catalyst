use super::{
    GROUP_MODEL_ANALYSIS_CONFIG_DIGEST_DOMAIN, GROUP_MODEL_ANALYSIS_EVENT_DIGEST_DOMAIN,
    GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT, GROUP_MODEL_ANALYSIS_REQUEST_DIGEST_DOMAIN,
    GROUP_MODEL_ANALYSIS_RESULT_DIGEST_DOMAIN, GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_DIGEST_DOMAIN,
    GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_VERSION, GROUP_MODEL_ANALYSIS_VERSION,
    GroupModelAnalysisConfig, GroupModelAnalysisEvent, GroupModelAnalysisJournalCursor,
    GroupModelAnalysisProvider, GroupModelAnalysisRecord, GroupModelAnalysisRequestConfig,
    GroupModelAnalysisResult, GroupModelAnalysisResultArtifact, HubEntity, HubStoreError,
    MAX_GROUP_MODEL_ANALYSIS_CONFIG_JSON_BYTES, MAX_GROUP_MODEL_ANALYSIS_CURSOR_JSON_BYTES,
    MAX_GROUP_MODEL_ANALYSIS_EVENT_JSON_BYTES, MAX_GROUP_MODEL_ANALYSIS_MODEL_BYTES,
    MAX_GROUP_MODEL_ANALYSIS_MODEL_EVENTS, MAX_GROUP_MODEL_ANALYSIS_OUTPUT_BYTES,
    MAX_GROUP_MODEL_ANALYSIS_OUTPUT_TOKENS, MAX_GROUP_MODEL_ANALYSIS_REQUEST_BYTES,
    MAX_GROUP_MODEL_ANALYSIS_RESULT_BYTES, MAX_GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_BYTES,
};

use super::{
    Cancellation, GroupRunSnapshot, Message, ModelRequest,
    group_context_build::{canonical_json_bytes, digest_with_domain_bytes},
    group_run_codec::{decode_hex_digest, encode_hex_digest, valid_text},
    openai_responses::OpenAiResponsesProvider,
};

pub(super) struct EncodedJson {
    pub json: String,
    pub digest: [u8; 32],
}

pub(super) fn encode_config(
    config: &GroupModelAnalysisRequestConfig,
) -> Result<EncodedJson, HubStoreError> {
    project_config(config)?;
    encode_bounded(
        config,
        GROUP_MODEL_ANALYSIS_CONFIG_DIGEST_DOMAIN,
        MAX_GROUP_MODEL_ANALYSIS_CONFIG_JSON_BYTES,
    )
}

pub(super) fn decode_config(
    json: &str,
    digest: &[u8; 32],
) -> Result<GroupModelAnalysisRequestConfig, HubStoreError> {
    let config = decode_bounded(
        json,
        digest,
        GROUP_MODEL_ANALYSIS_CONFIG_DIGEST_DOMAIN,
        MAX_GROUP_MODEL_ANALYSIS_CONFIG_JSON_BYTES,
        "configuration",
    )?;
    project_config(&config).map_err(|error| as_corrupt(&error))?;
    Ok(config)
}

pub(super) fn project_config(
    config: &GroupModelAnalysisRequestConfig,
) -> Result<GroupModelAnalysisConfig, HubStoreError> {
    validate_request_config(config)?;
    Ok(GroupModelAnalysisConfig {
        v: config.v,
        provider: config.provider,
        endpoint: config.endpoint.clone(),
        model: config.model.clone(),
        system_prompt_version: config.system_prompt_version,
        system_prompt_sha256: digest_hex(&system_prompt_digest(&config.system_prompt)),
        max_output_tokens: config.max_output_tokens,
        max_model_output_bytes: config.max_model_output_bytes,
        max_model_events: config.max_model_events,
    })
}

pub(super) fn encode_event(event: &GroupModelAnalysisEvent) -> Result<EncodedJson, HubStoreError> {
    encode_bounded(
        event,
        GROUP_MODEL_ANALYSIS_EVENT_DIGEST_DOMAIN,
        MAX_GROUP_MODEL_ANALYSIS_EVENT_JSON_BYTES,
    )
}

pub(super) fn decode_event(
    sequence: i64,
    json: &str,
    stored_digest: &[u8],
) -> Result<GroupModelAnalysisEvent, HubStoreError> {
    let digest = digest_from_blob(stored_digest, "event")?;
    let event: GroupModelAnalysisEvent = decode_bounded(
        json,
        &digest,
        GROUP_MODEL_ANALYSIS_EVENT_DIGEST_DOMAIN,
        MAX_GROUP_MODEL_ANALYSIS_EVENT_JSON_BYTES,
        "event",
    )?;
    let sequence = u64::try_from(sequence)
        .map_err(|error| corrupt(&format!("invalid analysis event sequence: {error}")))?;
    if event.seq == sequence {
        Ok(event)
    } else {
        Err(corrupt(
            "analysis event row sequence disagrees with its JSON",
        ))
    }
}

pub(super) fn encode_result(
    result: &GroupModelAnalysisResult,
    created_at_ms: u64,
) -> Result<(GroupModelAnalysisResultArtifact, EncodedJson), HubStoreError> {
    let bytes = serde_json::to_vec(result)
        .map_err(|error| corrupt(&format!("analysis result cannot encode: {error}")))?;
    let encoded = encoded_result(bytes)?;
    let artifact = GroupModelAnalysisResultArtifact {
        result: result.clone(),
        result_sha256: encode_hex_digest(&encoded.digest),
        result_bytes: encoded.json.len(),
        created_at_ms,
    };
    Ok((artifact, encoded))
}

pub(super) fn decode_result(
    bytes: &[u8],
    stored_digest: &[u8],
    created_at_ms: u64,
) -> Result<GroupModelAnalysisResultArtifact, HubStoreError> {
    let digest = digest_from_blob(stored_digest, "result")?;
    let json = std::str::from_utf8(bytes)
        .map_err(|error| corrupt(&format!("stored analysis result is not UTF-8: {error}")))?;
    validate_result_bytes(bytes, &digest)?;
    let result = serde_json::from_str(json)
        .map_err(|error| corrupt(&format!("invalid stored analysis result: {error}")))?;
    if serde_json::to_vec(&result).map_err(|error| corrupt(&error.to_string()))? != bytes {
        return Err(corrupt("stored analysis result is not canonical"));
    }
    Ok(GroupModelAnalysisResultArtifact {
        result,
        result_sha256: encode_hex_digest(&digest),
        result_bytes: bytes.len(),
        created_at_ms,
    })
}

fn encoded_result(bytes: Vec<u8>) -> Result<EncodedJson, HubStoreError> {
    if bytes.is_empty() || bytes.len() > MAX_GROUP_MODEL_ANALYSIS_RESULT_BYTES {
        return Err(conflict("canonical analysis result exceeds its byte limit"));
    }
    let digest = digest_with_domain_bytes(GROUP_MODEL_ANALYSIS_RESULT_DIGEST_DOMAIN, &bytes);
    let json = String::from_utf8(bytes)
        .map_err(|error| corrupt(&format!("analysis result is not UTF-8: {error}")))?;
    Ok(EncodedJson { json, digest })
}

fn validate_result_bytes(bytes: &[u8], digest: &[u8; 32]) -> Result<(), HubStoreError> {
    if bytes.is_empty() || bytes.len() > MAX_GROUP_MODEL_ANALYSIS_RESULT_BYTES {
        return Err(corrupt("stored analysis result is outside its byte bound"));
    }
    let actual = digest_with_domain_bytes(GROUP_MODEL_ANALYSIS_RESULT_DIGEST_DOMAIN, bytes);
    if actual == *digest {
        Ok(())
    } else {
        Err(corrupt("stored analysis result digest disagrees"))
    }
}

pub(super) fn encode_cursor(
    cursor: &GroupModelAnalysisJournalCursor,
) -> Result<String, HubStoreError> {
    let bytes = canonical_json_bytes(cursor)?;
    if bytes.is_empty() || bytes.len() > MAX_GROUP_MODEL_ANALYSIS_CURSOR_JSON_BYTES {
        return Err(conflict("analysis cursor exceeds its durable byte limit"));
    }
    String::from_utf8(bytes)
        .map_err(|error| corrupt(&format!("analysis cursor is not UTF-8: {error}")))
}

pub(super) fn decode_cursor(
    json: &str,
    record: &GroupModelAnalysisRecord,
) -> Result<GroupModelAnalysisJournalCursor, HubStoreError> {
    if json.is_empty() || json.len() > MAX_GROUP_MODEL_ANALYSIS_CURSOR_JSON_BYTES {
        return Err(corrupt("stored analysis cursor violates its byte bound"));
    }
    let cursor: GroupModelAnalysisJournalCursor = serde_json::from_str(json)
        .map_err(|error| corrupt(&format!("invalid stored analysis cursor: {error}")))?;
    cursor
        .validate_record(record)
        .map_err(|error| corrupt(&error.message))?;
    if encode_cursor(&cursor)? == json {
        Ok(cursor)
    } else {
        Err(corrupt("stored analysis cursor is not canonical"))
    }
}

pub(super) fn request_digest(body: &[u8]) -> Result<[u8; 32], HubStoreError> {
    if body.is_empty() || body.len() > MAX_GROUP_MODEL_ANALYSIS_REQUEST_BYTES {
        return Err(conflict(
            "Group model analysis request exceeds its durable byte limit",
        ));
    }
    Ok(digest_with_domain_bytes(
        GROUP_MODEL_ANALYSIS_REQUEST_DIGEST_DOMAIN,
        body,
    ))
}

pub(super) fn validate_exact_request(
    config: &GroupModelAnalysisRequestConfig,
    source: &GroupRunSnapshot,
    body: &[u8],
) -> Result<(), HubStoreError> {
    let expected = encode_exact_request(config, source).map_err(|error| as_corrupt(&error))?;
    if expected == body {
        Ok(())
    } else {
        Err(corrupt(
            "stored analysis request is not the exact bound provider request",
        ))
    }
}

pub(super) fn encode_exact_request(
    config: &GroupModelAnalysisRequestConfig,
    source: &GroupRunSnapshot,
) -> Result<Vec<u8>, HubStoreError> {
    validate_request_config(config)?;
    let request = ModelRequest {
        system_prompt: config.system_prompt.clone(),
        messages: vec![Message::User {
            text: source.context_json.clone(),
        }],
        tools: Vec::new(),
        max_output_tokens: config.max_output_tokens,
        cancellation: Cancellation::default(),
    };
    OpenAiResponsesProvider::encode_request_bytes(&config.model, &request)
        .map_err(|_| conflict("Group model analysis request could not be encoded"))
}

pub(super) fn system_prompt_digest(prompt: &str) -> [u8; 32] {
    digest_with_domain_bytes(
        GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_DIGEST_DOMAIN,
        prompt.as_bytes(),
    )
}

fn validate_request_config(config: &GroupModelAnalysisRequestConfig) -> Result<(), HubStoreError> {
    let valid = config.v == GROUP_MODEL_ANALYSIS_VERSION
        && config.provider == GroupModelAnalysisProvider::OpenAiResponses
        && config.endpoint == GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT
        && valid_text(&config.model, MAX_GROUP_MODEL_ANALYSIS_MODEL_BYTES)
        && config.system_prompt_version == GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_VERSION
        && valid_text(
            &config.system_prompt,
            MAX_GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_BYTES,
        )
        && (1..=MAX_GROUP_MODEL_ANALYSIS_OUTPUT_TOKENS).contains(&config.max_output_tokens)
        && (1..=MAX_GROUP_MODEL_ANALYSIS_OUTPUT_BYTES).contains(&config.max_model_output_bytes)
        && (1..=MAX_GROUP_MODEL_ANALYSIS_MODEL_EVENTS).contains(&config.max_model_events);
    valid
        .then_some(())
        .ok_or_else(|| conflict("Group model analysis configuration is invalid"))
}

pub(super) fn decode_digest(value: &[u8], subject: &str) -> Result<[u8; 32], HubStoreError> {
    digest_from_blob(value, subject)
}

pub(super) fn digest_hex(value: &[u8; 32]) -> String {
    encode_hex_digest(value)
}

pub(super) fn digest_bytes(value: &str, subject: &str) -> Result<[u8; 32], HubStoreError> {
    decode_hex_digest(value).ok_or_else(|| corrupt(&format!("invalid {subject} digest")))
}

fn encode_bounded<T: serde::Serialize>(
    value: &T,
    domain: &[u8],
    max_bytes: usize,
) -> Result<EncodedJson, HubStoreError> {
    let bytes = canonical_json_bytes(value)?;
    if bytes.is_empty() || bytes.len() > max_bytes {
        return Err(conflict("canonical analysis JSON exceeds its byte limit"));
    }
    let digest = digest_with_domain_bytes(domain, &bytes);
    let json = String::from_utf8(bytes)
        .map_err(|error| corrupt(&format!("analysis JSON is not UTF-8: {error}")))?;
    Ok(EncodedJson { json, digest })
}

fn decode_bounded<T: serde::de::DeserializeOwned + serde::Serialize>(
    json: &str,
    digest: &[u8; 32],
    domain: &[u8],
    max_bytes: usize,
    subject: &str,
) -> Result<T, HubStoreError> {
    if json.is_empty() || json.len() > max_bytes {
        return Err(corrupt(&format!(
            "stored analysis {subject} is outside its byte bound"
        )));
    }
    if digest_with_domain_bytes(domain, json.as_bytes()) != *digest {
        return Err(corrupt(&format!(
            "stored analysis {subject} digest disagrees"
        )));
    }
    let value = serde_json::from_str(json)
        .map_err(|error| corrupt(&format!("invalid stored analysis {subject}: {error}")))?;
    if canonical_json_bytes(&value)? == json.as_bytes() {
        Ok(value)
    } else {
        Err(corrupt(&format!(
            "stored analysis {subject} is not canonical"
        )))
    }
}

fn digest_from_blob(value: &[u8], subject: &str) -> Result<[u8; 32], HubStoreError> {
    value
        .try_into()
        .map_err(|_| corrupt(&format!("stored analysis {subject} digest is not 32 bytes")))
}

fn as_corrupt(error: &HubStoreError) -> HubStoreError {
    HubStoreError::Corrupt {
        message: error.to_string(),
    }
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupModelAnalysis,
        message: message.into(),
    }
}
