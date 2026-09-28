// Reviewed through 29b6356: these siblings consume supplied values and the
// Platform Core state graph, never AttemptRequest or AttemptLifecycle. Every
// source remains subject to the workspace consumer scan; this is no exemption.
pub(super) const REVIEWED_LEAVES: &[&str] = &[
    "attempt",
    "fabric",
    "lease",
    "prompt_append_receipt",
    "reconciliation",
    "run_attempt_lease_dispatch_preflight",
    "runner_attempt_boundary",
    "runner_command",
    "runner_execution_intent",
    "session_runner_receipt",
    "session_runner_receipt_history",
    "session_runner_reconciliation_projection",
];

// Keep this independent of the live source: new exports, re-exports, consumers
// and test registrations require review even at the existing module path.
pub(super) const DECLARATIONS: &str = "
pub mod attempt;
pub mod attempt_lifecycle;
pub mod fabric;
pub mod lease;
pub mod prompt_append_receipt;
pub mod reconciliation;
pub mod run_attempt_lease_dispatch_preflight;
pub mod runner_attempt_boundary;
pub mod runner_command;
pub mod runner_execution_intent;
pub mod session_runner_receipt;
pub mod session_runner_receipt_history;
pub mod session_runner_reconciliation_projection;
#[cfg(test)] mod attempt_lifecycle_contract;
#[cfg(test)] mod lease_checkpoint_contract;
#[cfg(test)] mod reconciliation_contract;
#[cfg(test)] mod run_attempt_lease_dispatch_preflight_contract;
#[cfg(test)] mod runner_attempt_boundary_contract;
#[cfg(test)] mod runner_command_digest_vectors_tests;
#[cfg(test)] mod runner_terminal_receipt_vectors_tests;
#[cfg(test)] mod session_runner_receipt_vectors_tests;
";
