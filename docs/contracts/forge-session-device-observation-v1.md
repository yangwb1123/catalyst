# Forge session device observation v1

`forge.session-device-observation/v1` is a bounded P3a display envelope. It
binds one caller-supplied owner tuple, Conversation, and Run to an offline
inventory declaration, a session placement observation, and a resource
summary.

The root contract has exactly these fields:

- `schema_version` and `evaluation_mode` are fixed to
  `forge.session-device-observation/v1` and `offline_static_only`.
- `owner`, `conversation_id`, `run_id`, and positive `evaluated_at_ms` carry
  the caller-declared binding. The owner must match every nested value.
- `owner_declaration_unverified` is `true`.
- `inventory` is the strict
  `forge.device-inventory-observation/v1` declaration page.
- `placement_observation` is the strict
  `forge.session-placement-observation/v1` projection. Decisions retain
  device/Runner instance pairs and are deterministically ordered.
- `resource_summary` is the strict
  `forge.device-resource-summary/v1` aggregate. Consumers recompute it from
  `inventory` and `placement_observation` before display.
- `selected_device_id` and `selected_instance_id` are always `null`.
- `authority` has the six placement authority fields, all `false`.

Consumers must reject unknown or duplicate fields, unexpected `null` values,
foreign owner/Conversation/Run bindings, unsorted or duplicate device/instance
rows, unsafe integers, inconsistent summary counts, selected targets, and any
authority bit. The envelope is not a registry snapshot and does not authorize
inventory, reservation, scheduling, dispatch, Runner, process execution, or
audit publication.

The canonical fixture is
`fixtures/forge-session-device-observation-v1.json`. Go and Flutter consume it
through `scripts/test-forge-contracts.sh`; Rust CLI/TUI consume the same fixture
through bounded local `FILE|-` commands. The authenticated Go preview route
produces the same envelope from caller-supplied declarations without registry,
clock, storage, or target-selection authority. Flutter's authenticated
`ForgeConversationsApi.previewSessionDeviceObservation` method posts the
caller-supplied session declaration once, then requires exact owner,
Conversation/Run, evaluation-time, and device/Runner binding before returning
the wire value. The shared Sessions screen may pass an explicit request to this
method; it only fetches after the selected Conversation/Run IDs match and
discards stale or foreign responses.
