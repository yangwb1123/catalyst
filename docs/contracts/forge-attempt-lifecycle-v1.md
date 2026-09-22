# Forge Attempt lifecycle v1

`forge.attempt-lifecycle/v1` freezes the authority-neutral Attempt state graph shared by Forge Core and forge-runtime. It covers only pure value validation and lifecycle reduction; it does not persist an Attempt, authenticate a caller, reserve a device, authorize execution, dispatch work, or publish an audit record.

The initial lifecycle value is `requested`. The declared edges are:

- `requested -> accepted`
- `accepted -> starting | interrupted | failed | uncertain`
- `starting -> running | interrupted | failed | uncertain`
- `running -> interrupted | completed | failed | uncertain`

`interrupted`, `completed`, `failed`, and `uncertain` are terminal in this v1 graph. Same-state transitions, undeclared edges, and unknown states reject with the stable Platform Core code `pc_transition_invalid` or `pc_state_invalid`.

The executable corpus is [forge-attempt-lifecycle-v1.json](fixtures/forge-attempt-lifecycle-v1.json). Go and Rust consumers decode it with unknown-field rejection and assert all 13 legal edges plus terminal and unknown-state rejection cases. `AttemptLifecycle::reduce` and `executionattempt.Lifecycle.Reduce` return a new value and leave the original value unchanged; they grant no execution authority.
