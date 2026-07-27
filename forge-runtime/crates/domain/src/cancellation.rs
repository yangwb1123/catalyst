use std::{
    future::poll_fn,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    task::{Poll, Waker},
};

#[derive(Clone, Debug, Default)]
pub struct Cancellation {
    state: Arc<CancellationState>,
}

#[derive(Debug, Default)]
struct CancellationState {
    cancelled: AtomicBool,
    wakers: Mutex<Vec<Waker>>,
}

impl Cancellation {
    pub fn cancel(&self) {
        if self.state.cancelled.swap(true, Ordering::AcqRel) {
            return;
        }
        let wakers = std::mem::take(&mut *self.wakers());
        for waker in wakers {
            waker.wake();
        }
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
    }

    pub async fn cancelled(&self) {
        poll_fn(|context| {
            if self.is_cancelled() {
                return Poll::Ready(());
            }
            let mut wakers = self.wakers();
            if self.is_cancelled() {
                return Poll::Ready(());
            }
            if !wakers.iter().any(|waker| waker.will_wake(context.waker())) {
                wakers.push(context.waker().clone());
            }
            Poll::Pending
        })
        .await;
    }

    fn wakers(&self) -> MutexGuard<'_, Vec<Waker>> {
        self.state
            .wakers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
