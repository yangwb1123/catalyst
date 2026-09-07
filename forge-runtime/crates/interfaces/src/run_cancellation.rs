use crate::runtime_domain::Cancellation;

#[cfg(unix)]
pub(super) fn cancellation_listener(cancellation: Cancellation) -> tokio::task::JoinHandle<()> {
    unix_listener(cancellation)
}

#[cfg(not(unix))]
pub(super) fn cancellation_listener(cancellation: Cancellation) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            cancellation.cancel();
        }
    })
}

#[cfg(unix)]
fn unix_listener(cancellation: Cancellation) -> tokio::task::JoinHandle<()> {
    let interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt());
    let terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate());
    if let (Ok(mut interrupt), Ok(mut terminate)) = (interrupt, terminate) {
        return tokio::spawn(async move {
            tokio::select! {
                _ = interrupt.recv() => {}
                _ = terminate.recv() => {}
            }
            cancellation.cancel();
        });
    }
    cancellation.cancel();
    tokio::spawn(async {})
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn listener_arms_before_returning() {
        let cancellation = Cancellation::default();
        let listener = cancellation_listener(cancellation.clone());
        assert!(!cancellation.is_cancelled());
        listener.abort();
        let _ = listener.await;
    }
}
