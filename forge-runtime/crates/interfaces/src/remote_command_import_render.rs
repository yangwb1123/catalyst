use std::fmt::Write as _;

use serde_json::Value;

use super::{LocalImportPreview, RemoteError, scope_label};

pub(crate) fn render_preview_text(preview: &LocalImportPreview) -> Result<String, RemoteError> {
    let source = &preview.source;
    let source_json = &preview.preview["source"];
    let target_json = &preview.preview["target"];
    let quoted = |value: &str| serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into());
    let mut output = String::new();
    writeln!(
        &mut output,
        "Local conversation import preview (nothing uploaded)"
    )
    .map_err(|_| RemoteError("could not render the local import preview".into()))?;
    writeln!(&mut output, "Source: this device's local Hub")
        .map_err(|_| RemoteError("could not render the local import preview".into()))?;
    writeln!(
        &mut output,
        "Conversation: {} {}",
        quoted(&source.conversation.id),
        quoted(&source.conversation.title)
    )
    .map_err(|_| RemoteError("could not render the local import preview".into()))?;
    writeln!(
        &mut output,
        "Scope: {}",
        scope_label(&source.conversation.scope)
    )
    .map_err(|_| RemoteError("could not render the local import preview".into()))?;
    writeln!(
        &mut output,
        "Visible prompts: {} ({} UTF-8 content bytes)",
        source_json["prompt_count"], source_json["content_bytes"]
    )
    .map_err(|_| RemoteError("could not render the local import preview".into()))?;
    writeln!(
        &mut output,
        "Target Coordinator: {}",
        target_json["coordinator"].as_str().unwrap_or("unknown")
    )
    .map_err(|_| RemoteError("could not render the local import preview".into()))?;
    writeln!(&mut output, "Destination scope: Global")
        .map_err(|_| RemoteError("could not render the local import preview".into()))?;
    writeln!(
        &mut output,
        "Target account preview: issuer={}, client={}, tenant={}, subject={}",
        quoted(target_json["issuer"].as_str().unwrap_or("")),
        quoted(target_json["client_id"].as_str().unwrap_or("")),
        quoted(target_json["tenant_id"].as_str().unwrap_or("")),
        quoted(target_json["subject"].as_str().unwrap_or(""))
    )
    .map_err(|_| RemoteError("could not render the local import preview".into()))?;
    writeln!(
        &mut output,
        "Account fields are decoded locally for preview; the Coordinator verifies the token before import."
    )
    .map_err(|_| RemoteError("could not render the local import preview".into()))?;
    for prompt in &source.prompts {
        let content = serde_json::to_string(&prompt.content)
            .map_err(|_| RemoteError("could not render the local import preview".into()))?;
        writeln!(&mut output, "\n[{}] {content}", prompt.role)
            .map_err(|_| RemoteError("could not render the local import preview".into()))?;
    }
    writeln!(&mut output, "\nConfirmation SHA-256: {}", preview.digest)
        .map_err(|_| RemoteError("could not render the local import preview".into()))?;
    writeln!(
        &mut output,
        "Confirm by repeating this command with --confirm {}.",
        preview.digest
    )
    .map_err(|_| RemoteError("could not render the local import preview".into()))?;
    Ok(output)
}

pub(crate) fn render_import_result(
    result: &Value,
    prompt_count: usize,
) -> Result<String, RemoteError> {
    let conversation_id = result["conversation"]["id"]
        .as_str()
        .ok_or_else(|| RemoteError("Forge API returned an invalid import result".into()))?;
    let replayed = result["replayed"].as_bool().unwrap_or(false);
    let mut output = String::new();
    writeln!(
        &mut output,
        "{} shared conversation {} ({} visible prompts).{}",
        if replayed {
            "Confirmed existing"
        } else {
            "Imported"
        },
        conversation_id,
        prompt_count,
        if replayed {
            " This was an idempotent retry."
        } else {
            ""
        },
    )
    .map_err(|_| RemoteError("could not render the import result".into()))?;
    writeln!(
        &mut output,
        "The original local conversation was left unchanged."
    )
    .map_err(|_| RemoteError("could not render the import result".into()))?;
    Ok(output)
}
