use serde_json::Value;

use super::{RemoteClient, RemoteError};

impl RemoteClient {
    /// Reads one validated metadata-only Run timeline page from the saved
    /// owner/Run checkpoint and advances that checkpoint only after success.
    /// Explicit bearer-token sessions cannot resume because they have no
    /// persistent owner-bound credential store.
    pub(super) async fn resumed_run_timeline(
        &self,
        conversation_id: &str,
        run_id: &str,
        limit: usize,
    ) -> Result<Value, RemoteError> {
        let checkpoint = self
            .change_cursor
            .as_ref()
            .ok_or_else(|| RemoteError("Run timeline resume requires a saved Forge login".into()))?
            .run_timeline_cursor_store(conversation_id, run_id);
        let after_sequence = checkpoint.load().map_err(RemoteError)?;
        let page = self
            .run_timeline(conversation_id, run_id, after_sequence, limit)
            .await?;
        let scanned_through_sequence = page
            .get("scanned_through_sequence")
            .and_then(Value::as_u64)
            .ok_or_else(|| RemoteError("Run timeline checkpoint metadata is invalid".into()))?;
        if !checkpoint
            .save(scanned_through_sequence)
            .map_err(RemoteError)?
        {
            return Err(RemoteError(
                "Run timeline checkpoint would move backwards".into(),
            ));
        }
        Ok(page)
    }
}
