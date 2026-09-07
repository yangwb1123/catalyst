use crate::runtime_domain::RunLimits;

const MAX_AGENT_MODEL_EVENTS: u32 = 2_048;
const MAX_AGENT_TOOL_OUTPUT_BYTES: usize = 128 * 1024;
const TOOL_OUTPUT_JOURNAL_BUDGET: usize = 32 * 1024 * 1024;
const JSON_STRING_EXPANSION: usize = 6;
const TOOL_OUTPUT_JOURNAL_COPIES: usize = 2;

pub(super) fn bounded(max_turns: u32, max_tool_calls: u32, max_output_tokens: u32) -> RunLimits {
    RunLimits {
        max_turns,
        max_tool_calls,
        max_tool_output_bytes: tool_output_limit(max_tool_calls),
        max_model_output_bytes: 256 * 1024,
        max_model_events: MAX_AGENT_MODEL_EVENTS,
        max_output_tokens_per_turn: max_output_tokens,
    }
}

fn tool_output_limit(max_tool_calls: u32) -> usize {
    if max_tool_calls == 0 {
        return MAX_AGENT_TOOL_OUTPUT_BYTES;
    }
    let calls = usize::try_from(max_tool_calls).unwrap_or(usize::MAX);
    let bytes_per_input_byte = JSON_STRING_EXPANSION * TOOL_OUTPUT_JOURNAL_COPIES;
    TOOL_OUTPUT_JOURNAL_BUDGET
        .checked_div(calls.saturating_mul(bytes_per_input_byte))
        .unwrap_or(0)
        .min(MAX_AGENT_TOOL_OUTPUT_BYTES)
}

#[cfg(test)]
#[path = "agent_run_limits_tests.rs"]
mod tests;
