//! Immutable, caller-supplied Attempt request declarations.
//!
//! Validation here resolves no reference and releases no execution authority.

mod error;
mod model;
mod validation;

pub use error::{AttemptRequestError, AttemptRequestErrorCode};
pub use model::{AttemptBudget, AttemptRequest, AttemptRequestInput, ControlVersionBinding};
pub use validation::{
    APPROVAL_RECORD_TYPE, CAPABILITY_GRANT_RECORD_TYPE, MAX_APPROVAL_REFS,
    MAX_ATTEMPT_COST_USD_MICROS, MAX_ATTEMPT_DURATION_MS, MAX_ATTEMPT_INPUT_TOKENS,
    MAX_ATTEMPT_MODEL_CALLS, MAX_ATTEMPT_NETWORK_BYTES, MAX_ATTEMPT_OUTPUT_BYTES,
    MAX_ATTEMPT_OUTPUT_TOKENS, MAX_ATTEMPT_TIMEOUT_MS, MAX_ATTEMPT_TOOL_CALLS,
    MAX_CONTROL_AGGREGATE_VERSION, MAX_REQUESTED_EFFECT_BYTES, MAX_REQUESTED_EFFECTS,
    WORKSPACE_CAPABILITY_RECORD_TYPE,
};
