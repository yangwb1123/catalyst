# AI Agent Application Profile

## Goal

Let users express a goal, observe execution, provide corrections, understand
evidence, and safely approve consequential tool actions.

## Layout and density

- Standard density with conversation, task state, tool activity, artifacts,
  and input kept visually distinct.
- Long-running work exposes stage progress and the current blocking condition.
- Results identify evidence and separate verified facts from assumptions.

## Interaction requirements

- Distinguish thinking, executing, waiting for input, blocked, failed,
  cancelled, and completed states.
- Users can interrupt, revise, retry, or resume when the underlying action
  semantics permit it.
- High-risk writes require an explicit preview and confirmation.
- Tool failures identify the failed step and preserve completed evidence.
