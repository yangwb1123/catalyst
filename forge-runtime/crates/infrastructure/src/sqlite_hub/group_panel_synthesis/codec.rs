use super::{
    Cancellation, GROUP_PANEL_SYNTHESIS_CONFIG_DIGEST_DOMAIN,
    GROUP_PANEL_SYNTHESIS_EVENT_DIGEST_DOMAIN, GROUP_PANEL_SYNTHESIS_PROVIDER_ENDPOINT,
    GROUP_PANEL_SYNTHESIS_REQUEST_DIGEST_DOMAIN, GROUP_PANEL_SYNTHESIS_RESULT_DIGEST_DOMAIN,
    GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_DIGEST_DOMAIN, GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_VERSION,
    GROUP_PANEL_SYNTHESIS_VERSION, GroupAnalysisPanelInspection, GroupPanelSynthesisConfig,
    GroupPanelSynthesisEvent, GroupPanelSynthesisJournalCursor, GroupPanelSynthesisOutputTarget,
    GroupPanelSynthesisProvider, GroupPanelSynthesisRecord, GroupPanelSynthesisRequestConfig,
    GroupPanelSynthesisResult, GroupPanelSynthesisResultArtifact,
    GroupPanelSynthesisWritebackTarget, HubEntity, HubStoreError,
    MAX_GROUP_PANEL_SYNTHESIS_CONFIG_JSON_BYTES, MAX_GROUP_PANEL_SYNTHESIS_CURSOR_JSON_BYTES,
    MAX_GROUP_PANEL_SYNTHESIS_EVENT_JSON_BYTES, MAX_GROUP_PANEL_SYNTHESIS_MODEL_BYTES,
    MAX_GROUP_PANEL_SYNTHESIS_MODEL_EVENTS, MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_BYTES,
    MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_TOKENS, MAX_GROUP_PANEL_SYNTHESIS_REQUEST_BYTES,
    MAX_GROUP_PANEL_SYNTHESIS_RESULT_BYTES, MAX_GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_BYTES, Message,
    ModelRequest,
    group_context_build::{canonical_json_bytes, digest_with_domain_bytes},
    group_run_codec::{decode_hex_digest, encode_hex_digest, valid_text},
    openai_responses::OpenAiResponsesProvider,
};

pub(super) struct EncodedJson {
    pub json: String,
    pub digest: [u8; 32],
}

pub(super) fn encode_config(
    config: &GroupPanelSynthesisRequestConfig,
) -> Result<EncodedJson, HubStoreError> {
    project_config(config)?;
    encode_bounded(
        config,
        GROUP_PANEL_SYNTHESIS_CONFIG_DIGEST_DOMAIN,
        MAX_GROUP_PANEL_SYNTHESIS_CONFIG_JSON_BYTES,
    )
}

pub(super) fn decode_config(
    json: &str,
    digest: &[u8; 32],
) -> Result<GroupPanelSynthesisRequestConfig, HubStoreError> {
    let config = decode_bounded(
        json,
        digest,
        GROUP_PANEL_SYNTHESIS_CONFIG_DIGEST_DOMAIN,
        MAX_GROUP_PANEL_SYNTHESIS_CONFIG_JSON_BYTES,
        "configuration",
    )?;
    project_config(&config).map_err(|error| as_corrupt(&error))?;
    Ok(config)
}

pub(super) fn project_config(
    config: &GroupPanelSynthesisRequestConfig,
) -> Result<GroupPanelSynthesisConfig, HubStoreError> {
    validate_request_config(config)?;
    Ok(GroupPanelSynthesisConfig {
        v: config.v,
        provider: config.provider,
        endpoint: config.endpoint.clone(),
        model: config.model.clone(),
        system_prompt_version: config.system_prompt_version,
        system_prompt_sha256: digest_hex(&system_prompt_digest(&config.system_prompt)),
        max_output_tokens: config.max_output_tokens,
        max_model_output_bytes: config.max_model_output_bytes,
        max_model_events: config.max_model_events,
        output_target: config.output_target,
        writeback_target: config.writeback_target,
    })
}

pub(super) fn encode_event(event: &GroupPanelSynthesisEvent) -> Result<EncodedJson, HubStoreError> {
    encode_bounded(
        event,
        GROUP_PANEL_SYNTHESIS_EVENT_DIGEST_DOMAIN,
        MAX_GROUP_PANEL_SYNTHESIS_EVENT_JSON_BYTES,
    )
}

pub(super) fn decode_event(
    sequence: i64,
    json: &str,
    stored_digest: &[u8],
) -> Result<GroupPanelSynthesisEvent, HubStoreError> {
    let digest = digest_from_blob(stored_digest, "event")?;
    let event: GroupPanelSynthesisEvent = decode_bounded(
        json,
        &digest,
        GROUP_PANEL_SYNTHESIS_EVENT_DIGEST_DOMAIN,
        MAX_GROUP_PANEL_SYNTHESIS_EVENT_JSON_BYTES,
        "event",
    )?;
    let sequence = u64::try_from(sequence)
        .map_err(|error| corrupt(&format!("invalid synthesis event sequence: {error}")))?;
    if event.seq == sequence {
        Ok(event)
    } else {
        Err(corrupt(
            "synthesis event row sequence disagrees with its JSON",
        ))
    }
}

pub(super) fn encode_result(
    result: &GroupPanelSynthesisResult,
    created_at_ms: u64,
) -> Result<(GroupPanelSynthesisResultArtifact, EncodedJson), HubStoreError> {
    let bytes = canonical_json_bytes(result)?;
    let encoded = encoded_result(bytes)?;
    let artifact = GroupPanelSynthesisResultArtifact {
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
) -> Result<GroupPanelSynthesisResultArtifact, HubStoreError> {
    let digest = digest_from_blob(stored_digest, "result")?;
    let json = std::str::from_utf8(bytes)
        .map_err(|error| corrupt(&format!("stored synthesis result is not UTF-8: {error}")))?;
    validate_result_bytes(bytes, &digest)?;
    let result = serde_json::from_str(json)
        .map_err(|error| corrupt(&format!("invalid stored synthesis result: {error}")))?;
    if canonical_json_bytes(&result)? != bytes {
        return Err(corrupt("stored synthesis result is not canonical"));
    }
    Ok(GroupPanelSynthesisResultArtifact {
        result,
        result_sha256: encode_hex_digest(&digest),
        result_bytes: bytes.len(),
        created_at_ms,
    })
}

fn encoded_result(bytes: Vec<u8>) -> Result<EncodedJson, HubStoreError> {
    if bytes.is_empty() || bytes.len() > MAX_GROUP_PANEL_SYNTHESIS_RESULT_BYTES {
        return Err(conflict(
            "canonical synthesis result exceeds its byte limit",
        ));
    }
    let digest = digest_with_domain_bytes(GROUP_PANEL_SYNTHESIS_RESULT_DIGEST_DOMAIN, &bytes);
    let json = String::from_utf8(bytes)
        .map_err(|error| corrupt(&format!("synthesis result is not UTF-8: {error}")))?;
    Ok(EncodedJson { json, digest })
}

fn validate_result_bytes(bytes: &[u8], digest: &[u8; 32]) -> Result<(), HubStoreError> {
    if bytes.is_empty() || bytes.len() > MAX_GROUP_PANEL_SYNTHESIS_RESULT_BYTES {
        return Err(corrupt("stored synthesis result is outside its byte bound"));
    }
    let actual = digest_with_domain_bytes(GROUP_PANEL_SYNTHESIS_RESULT_DIGEST_DOMAIN, bytes);
    if actual == *digest {
        Ok(())
    } else {
        Err(corrupt("stored synthesis result digest disagrees"))
    }
}

pub(super) fn encode_cursor(
    cursor: &GroupPanelSynthesisJournalCursor,
) -> Result<String, HubStoreError> {
    let bytes = canonical_json_bytes(cursor)?;
    if bytes.is_empty() || bytes.len() > MAX_GROUP_PANEL_SYNTHESIS_CURSOR_JSON_BYTES {
        return Err(conflict("synthesis cursor exceeds its durable byte limit"));
    }
    String::from_utf8(bytes)
        .map_err(|error| corrupt(&format!("synthesis cursor is not UTF-8: {error}")))
}

pub(super) fn decode_cursor(
    json: &str,
    record: &GroupPanelSynthesisRecord,
) -> Result<GroupPanelSynthesisJournalCursor, HubStoreError> {
    if json.is_empty() || json.len() > MAX_GROUP_PANEL_SYNTHESIS_CURSOR_JSON_BYTES {
        return Err(corrupt("stored synthesis cursor violates its byte bound"));
    }
    let cursor: GroupPanelSynthesisJournalCursor = serde_json::from_str(json)
        .map_err(|error| corrupt(&format!("invalid stored synthesis cursor: {error}")))?;
    cursor
        .validate_record(record)
        .map_err(|error| corrupt(&error.message))?;
    if encode_cursor(&cursor)? == json {
        Ok(cursor)
    } else {
        Err(corrupt("stored synthesis cursor is not canonical"))
    }
}

pub(super) fn request_digest(body: &[u8]) -> Result<[u8; 32], HubStoreError> {
    if body.is_empty() || body.len() > MAX_GROUP_PANEL_SYNTHESIS_REQUEST_BYTES {
        return Err(conflict(
            "Group Panel Synthesis request exceeds its durable byte limit",
        ));
    }
    Ok(digest_with_domain_bytes(
        GROUP_PANEL_SYNTHESIS_REQUEST_DIGEST_DOMAIN,
        body,
    ))
}

pub(super) fn validate_exact_request(
    config: &GroupPanelSynthesisRequestConfig,
    panel: &GroupAnalysisPanelInspection,
    body: &[u8],
) -> Result<(), HubStoreError> {
    let expected = encode_exact_request(config, panel).map_err(|error| as_corrupt(&error))?;
    if expected == body {
        Ok(())
    } else {
        Err(corrupt(
            "stored synthesis request is not the exact bound provider request",
        ))
    }
}

pub(super) fn encode_exact_request(
    config: &GroupPanelSynthesisRequestConfig,
    panel: &GroupAnalysisPanelInspection,
) -> Result<Vec<u8>, HubStoreError> {
    validate_request_config(config)?;
    let bytes = canonical_json_bytes(&panel.manifest)?;
    let text = String::from_utf8(bytes)
        .map_err(|error| corrupt(&format!("panel manifest is not UTF-8: {error}")))?;
    let request = ModelRequest {
        system_prompt: config.system_prompt.clone(),
        messages: vec![Message::User { text }],
        tools: Vec::new(),
        max_output_tokens: config.max_output_tokens,
        cancellation: Cancellation::default(),
    };
    OpenAiResponsesProvider::encode_request_bytes(&config.model, &request)
        .map_err(|_| conflict("Group Panel Synthesis request could not be encoded"))
}

pub(super) fn system_prompt_digest(prompt: &str) -> [u8; 32] {
    digest_with_domain_bytes(
        GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_DIGEST_DOMAIN,
        prompt.as_bytes(),
    )
}

fn validate_request_config(config: &GroupPanelSynthesisRequestConfig) -> Result<(), HubStoreError> {
    let valid = config.v == GROUP_PANEL_SYNTHESIS_VERSION
        && config.provider == GroupPanelSynthesisProvider::OpenAiResponses
        && config.endpoint == GROUP_PANEL_SYNTHESIS_PROVIDER_ENDPOINT
        && valid_text(&config.model, MAX_GROUP_PANEL_SYNTHESIS_MODEL_BYTES)
        && config.system_prompt_version == GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_VERSION
        && valid_text(
            &config.system_prompt,
            MAX_GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_BYTES,
        )
        && (1..=MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_TOKENS).contains(&config.max_output_tokens)
        && (1..=MAX_GROUP_PANEL_SYNTHESIS_OUTPUT_BYTES).contains(&config.max_model_output_bytes)
        && (1..=MAX_GROUP_PANEL_SYNTHESIS_MODEL_EVENTS).contains(&config.max_model_events)
        && config.output_target == GroupPanelSynthesisOutputTarget::LocalArtifact
        && config.writeback_target == GroupPanelSynthesisWritebackTarget::None;
    valid
        .then_some(())
        .ok_or_else(|| conflict("Group Panel Synthesis configuration is invalid"))
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
        return Err(conflict("canonical synthesis JSON exceeds its byte limit"));
    }
    let digest = digest_with_domain_bytes(domain, &bytes);
    let json = String::from_utf8(bytes)
        .map_err(|error| corrupt(&format!("synthesis JSON is not UTF-8: {error}")))?;
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
            "stored synthesis {subject} is outside its byte bound"
        )));
    }
    if digest_with_domain_bytes(domain, json.as_bytes()) != *digest {
        return Err(corrupt(&format!(
            "stored synthesis {subject} digest disagrees"
        )));
    }
    let value = serde_json::from_str(json)
        .map_err(|error| corrupt(&format!("invalid stored synthesis {subject}: {error}")))?;
    if canonical_json_bytes(&value)? == json.as_bytes() {
        Ok(value)
    } else {
        Err(corrupt(&format!(
            "stored synthesis {subject} is not canonical"
        )))
    }
}

fn digest_from_blob(value: &[u8], subject: &str) -> Result<[u8; 32], HubStoreError> {
    value.try_into().map_err(|_| {
        corrupt(&format!(
            "stored synthesis {subject} digest is not 32 bytes"
        ))
    })
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
        entity: HubEntity::GroupPanelSynthesis,
        message: message.into(),
    }
}
