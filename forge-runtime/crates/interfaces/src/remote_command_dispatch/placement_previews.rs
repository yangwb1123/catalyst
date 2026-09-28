use super::{Error, RemoteClient, RemoteCommand, Value};

pub(super) async fn execute(
    client: &RemoteClient,
    command: &RemoteCommand,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::PlacementPreview { input } => placement(client, input).await,
        RemoteCommand::PlacementRegistryPreview { input } => registry(client, input).await,
        RemoteCommand::CredentialCandidatePreview { input } => credential(client, input).await,
        _ => unreachable!("placement preview command was classified before dispatch"),
    }
}

async fn placement(client: &RemoteClient, input: &str) -> Result<Value, Box<dyn Error>> {
    let request = super::super::placement::read_request(input)?;
    let response = client.preview_device_placement(&request).await?;
    super::super::placement::validate_response(&response, &request)?;
    Ok(response)
}

async fn registry(client: &RemoteClient, input: &str) -> Result<Value, Box<dyn Error>> {
    let request = super::super::placement_registry::read_request(input)?;
    let response = client.preview_device_placement_registry(&request).await?;
    super::super::placement_registry::validate_response(&response)?;
    Ok(response)
}

async fn credential(client: &RemoteClient, input: &str) -> Result<Value, Box<dyn Error>> {
    let request = crate::remote_credential_candidate::read_request(input)?;
    let response = client.preview_device_credential_candidate(&request).await?;
    crate::remote_credential_candidate::validate_response(&response, &request)?;
    Ok(response)
}
