use super::changes::OwnedConversationChangePage;
use super::{RemoteClient, RemoteError};

impl RemoteClient {
    pub(super) fn saved_change_cursor(&self) -> Result<u64, RemoteError> {
        self.change_cursor
            .as_ref()
            .map_or(Ok(0), |store| store.load().map_err(RemoteError))
    }

    pub(super) fn persist_change_cursor(&self, cursor: u64) -> Result<(), RemoteError> {
        if let Some(store) = &self.change_cursor {
            store.save(cursor).map_err(RemoteError)?;
        }
        Ok(())
    }

    pub(super) async fn resumed_conversation_changes(
        &self,
        explicit_after: Option<u64>,
    ) -> Result<OwnedConversationChangePage, RemoteError> {
        let after_cursor = explicit_after.map_or_else(|| self.saved_change_cursor(), Ok)?;
        let page = self.conversation_changes_after(after_cursor).await?;
        if explicit_after.is_none() {
            self.persist_change_cursor(page.scanned_through_cursor)?;
        }
        Ok(page)
    }
}
