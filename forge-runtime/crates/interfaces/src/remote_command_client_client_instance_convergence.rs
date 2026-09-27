use serde_json::Value;

use super::{RemoteClient, RemoteError};

impl RemoteClient {
    /// Reads the owner-bound session and resource views as one explicit pair.
    /// Both requests are GET-only; owner or instance-row drift fails closed.
    pub(crate) async fn read_converged_client_instance_views(&self) -> Result<Value, RemoteError> {
        let session_view = self.read_client_instance_session_view().await?;
        let resource_view = self.read_client_instance_resource_view().await?;
        super::client_instance_convergence::validate(&session_view, &resource_view)?;
        Ok(super::client_instance_convergence::envelope(
            session_view,
            resource_view,
        ))
    }
}
