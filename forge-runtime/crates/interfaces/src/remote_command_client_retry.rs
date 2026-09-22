use std::time::Duration;

use super::{READ_RETRY_BACKOFF_MS, RemoteError};

pub(super) fn is_transient_read_error(error: &RemoteError) -> bool {
    let status = error
        .0
        .strip_prefix("Forge API returned HTTP ")
        .and_then(|value| value.get(..3))
        .and_then(|value| value.parse::<u16>().ok());
    status.is_some_and(|status| matches!(status, 408 | 425 | 429 | 500..=599))
        || matches!(
            error.0.as_str(),
            "Forge API request failed" | "Forge API response could not be read"
        )
}

pub(super) async fn retry_request_and_wait(
    retry_request: Option<reqwest::RequestBuilder>,
    attempt: usize,
) -> reqwest::RequestBuilder {
    let Some(next_request) = retry_request else {
        unreachable!("a retry is only scheduled when a clone is available")
    };
    let delay_ms = READ_RETRY_BACKOFF_MS
        .get(attempt)
        .copied()
        .unwrap_or_default();
    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    next_request
}
