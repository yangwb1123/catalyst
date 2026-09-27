use serde_json::Value;

use super::{RemoteClient, RemoteError};

impl RemoteClient {
    /// Reads the owner-scoped v2 inventory and composed resource view as one
    /// explicit pair. Both source reads are GET-only; a counter or identity
    /// drift fails closed before an envelope is returned.
    pub(crate) async fn read_converged_inventory_resource_view(
        &self,
    ) -> Result<Value, RemoteError> {
        let inventory = self.read_device_inventory_v2().await?;
        let resource_view = self.read_client_instance_resource_view().await?;
        super::super::inventory_convergence::validate(&inventory, &resource_view)?;
        Ok(super::super::inventory_convergence::envelope(
            inventory,
            resource_view,
        ))
    }
}
