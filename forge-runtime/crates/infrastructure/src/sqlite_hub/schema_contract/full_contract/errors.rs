use super::HubStoreError;

pub(super) fn invalid(version: i64, object: &str, detail: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: format!("Hub v{version} {object} has invalid {detail}"),
    }
}

pub(super) fn unavailable(error: impl std::fmt::Display) -> HubStoreError {
    HubStoreError::Unavailable {
        message: error.to_string(),
    }
}

pub(super) fn stringify(error: impl std::fmt::Display) -> String {
    error.to_string()
}
