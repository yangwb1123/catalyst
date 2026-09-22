# Forge Run intent observation contract v1

`forgeos.run-intent-observation-contract/v1` is a value-only bridge between
the authenticated owner's prompt acceptance receipt, an already observed Run
summary, and the existing `forge.session-placement-observation/v1` result.
It lets CLI, TUI, Web, App, and Mobile consumers explain how one shared
Conversation prompt relates to the current offline device declarations.

The input contains an owner tuple, Conversation ID, payload-free prompt receipt,
Run reference, and an existing session-placement observation. The observer
requires exact Conversation/Prompt/Run identity links, the initial `submitted`
event envelope, a known Run status, and an owner/run match in placement. It
returns only binding metadata and counts of placement decisions and eligible
declared Runner instances. Prompt content is absent from this contract.

`preview_only` is always `true`; `selected_device_id` and
`selected_instance_id` are always `null`. Owner and device declarations remain
unverified. Every placement authority bit is fixed to `false`, including
`execution_authorized`, `reservation_created`, and `dispatch_performed`.

This contract does not create a prompt or Run, read a clock, contact Hub or
Runner, persist data, discover/register/heartbeat devices, select a target,
reserve capacity, authorize execution, dispatch a command, or execute a
process. The placement input must already satisfy the offline session
placement contract. ADR-0039 remains planning-only, ADR-0114 remains Proposed,
and P4 still requires a separately Accepted execution/security decision before
any authority-bearing path is exposed.
