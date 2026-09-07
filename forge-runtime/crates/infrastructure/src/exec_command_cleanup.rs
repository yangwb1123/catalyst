use std::{
    process::{Child, ExitStatus},
    thread,
    time::{Duration, Instant},
};

use crate::runtime_domain::{Cancellation, TOOL_EFFECT_UNCERTAIN_CODE, ToolError};

const POLL_INTERVAL: Duration = Duration::from_millis(5);
const KILL_REAP_GRACE: Duration = Duration::from_secs(1);

pub(super) fn wait_for_child(
    child: &mut Child,
    cancellation: &Cancellation,
    deadline: Instant,
) -> Result<ExitStatus, ToolError> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(error) => {
                let failure = ToolError::new("wait_failed", error.to_string());
                return Err(cleanup_after_error(child, failure));
            }
        }
        if cancellation.is_cancelled() {
            return Err(cleanup_after_error(child, cancelled_error()));
        }
        if Instant::now() >= deadline {
            let timeout = ToolError::new("timed_out", "command exceeded timeout_ms");
            return Err(cleanup_after_error(child, timeout));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

pub(super) fn cleanup_after_error(child: &mut Child, _error: ToolError) -> ToolError {
    // Killing and reaping the original process group cannot prove that a
    // descendant did not create a new session and continue running. Once the
    // command was spawned, every abnormal exit is therefore effect-uncertain.
    let _ = terminate_and_reap(child);
    ToolError::new(
        TOOL_EFFECT_UNCERTAIN_CODE,
        "command ended abnormally after spawn; descendant cleanup and effects could not be confirmed",
    )
}

pub(super) fn cleanup_after_capture_error(_child: &mut Child, _error: ToolError) -> ToolError {
    // The direct child has already reached a terminal state when output is
    // drained and `try_wait` has reaped it. Its numeric PID/PGID is no longer
    // an ownership token and may already identify an unrelated process group,
    // so this path must never signal it. An open pipe proves only that some
    // other process retained the descriptor; report uncertainty without a
    // blind post-reap kill.
    ToolError::new(
        TOOL_EFFECT_UNCERTAIN_CODE,
        "command output remained open after the direct child exited; descendant cleanup could not be confirmed",
    )
}

fn terminate_and_reap(child: &mut Child) -> bool {
    #[cfg(unix)]
    let group = rustix::process::Pid::from_child(&*child);
    terminate_process_group(child);
    let _ = child.kill();
    let deadline = Instant::now() + KILL_REAP_GRACE;
    let mut child_reaped = false;
    while Instant::now() < deadline {
        if !child_reaped {
            match child.try_wait() {
                Ok(Some(_)) => child_reaped = true,
                Ok(None) => {}
                Err(_) => return false,
            }
        }
        #[cfg(unix)]
        let owned_process_group_gone = matches!(
            rustix::process::test_kill_process_group(group),
            Err(rustix::io::Errno::SRCH)
        );
        #[cfg(not(unix))]
        let owned_process_group_gone = true;
        if child_reaped && owned_process_group_gone {
            return true;
        }
        thread::sleep(POLL_INTERVAL);
    }
    false
}

pub(super) fn cancelled_error() -> ToolError {
    ToolError::new("cancelled", "run was cancelled during command execution")
}

#[cfg(unix)]
fn terminate_process_group(child: &Child) {
    let group = rustix::process::Pid::from_child(child);
    let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
}

#[cfg(not(unix))]
fn terminate_process_group(_child: &Child) {}
