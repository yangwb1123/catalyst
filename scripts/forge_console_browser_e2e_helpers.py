"""Authenticated observation helpers for the Forge Console browser E2E."""

import json


def post_session_device_observation(
    page, access_token: str, conversation_id: str, run_id: str, request: dict
) -> dict:
    """Post the bounded P3a declaration and validate the canonical response."""
    path = (
        f"/api/v1/conversations/{conversation_id}/runs/{run_id}/"
        "device-observation/preview"
    )
    result = page.evaluate(
        """
        async ({path, accessToken, request}) => {
          const response = await fetch(path, {
            method: 'POST',
            headers: {
              'Authorization': `Bearer ${accessToken}`,
              'Content-Type': 'application/json',
            },
            body: JSON.stringify(request),
          });
          return {status: response.status, body: await response.text()};
        }
        """,
        {"path": path, "accessToken": access_token, "request": request},
    )
    if result["status"] != 200:
        raise AssertionError(
            "Forge Web session device observation preview returned "
            f"{result['status']}: {result['body']}"
        )
    returned = _decode_json_response(
        result["body"], "Forge Web session device observation preview"
    )
    if returned.get("owner") != request.get("owner"):
        raise AssertionError(
            "Forge Web session device observation owner drifted: "
            f"request={request.get('owner')!r} response={returned.get('owner')!r}"
        )
    if (
        returned.get("conversation_id") != conversation_id
        or returned.get("run_id") != run_id
        or returned.get("selected_device_id") is not None
        or returned.get("selected_instance_id") is not None
    ):
        raise AssertionError(
            "Forge Web session device observation lost its Run binding: "
            f"{returned!r}"
        )
    authority = returned.get("authority")
    if not isinstance(authority, dict) or any(authority.values()):
        raise AssertionError(
            "Forge Web session device observation exposed authority: "
            f"{authority!r}"
        )
    placement = request.get("placement")
    inventory = returned.get("inventory")
    placement_observation = returned.get("placement_observation")
    summary = returned.get("resource_summary")
    candidates = request.get("candidates")
    returned_devices = inventory.get("devices") if isinstance(inventory, dict) else None
    if (
        not isinstance(placement, dict)
        or not isinstance(inventory, dict)
        or not isinstance(placement_observation, dict)
        or not isinstance(summary, dict)
        or not isinstance(candidates, list)
        or not isinstance(returned_devices, list)
        or len(candidates) != 9
        or len(returned_devices) != len(candidates)
        or returned.get("evaluated_at_ms") != placement.get("evaluated_at_ms")
        or inventory.get("evaluated_at_ms") != placement.get("evaluated_at_ms")
        or placement_observation.get("evaluated_at_ms")
        != placement.get("evaluated_at_ms")
        or summary.get("evaluated_at_ms") != placement.get("evaluated_at_ms")
        or canonical_json_values(returned_devices) != canonical_json_values(candidates)
    ):
        raise AssertionError(
            "Forge Web session device observation declaration drifted: "
            f"request={request!r} response={returned!r}"
        )
    decisions = placement_observation.get("decisions")
    candidate_keys = {
        (candidate.get("device", {}).get("device_id"), candidate.get("instance_id"))
        for candidate in candidates
        if isinstance(candidate, dict) and isinstance(candidate.get("device"), dict)
    }
    decision_keys = (
        {
            (decision.get("device_id"), decision.get("instance_id"))
            for decision in decisions
            if isinstance(decision, dict)
        }
        if isinstance(decisions, list)
        else set()
    )
    if len(candidate_keys) != len(candidates) or decision_keys != candidate_keys:
        raise AssertionError(
            "Forge Web session device observation placement drifted: "
            f"candidates={candidate_keys!r} decisions={decision_keys!r}"
        )
    expected_summary = {
        "device_count": 9,
        "runner_instance_count": 9,
        "available_cpu_cores": 66,
        "available_memory_bytes": 135168,
        "available_storage_bytes": 67584,
        "available_gpu_count": 0,
        "available_gpu_memory_bytes": 0,
        "eligible_device_count": 2,
        "eligible_instance_count": 2,
    }
    if any(summary.get(key) != value for key, value in expected_summary.items()):
        raise AssertionError(
            "Forge Web session device observation summary drifted: "
            f"{summary!r}"
        )
    return returned


def post_session_runner_receipt_observation(
    page, access_token: str, conversation_id: str, run_id: str, observation: dict
) -> None:
    """Post the bounded receipt envelope once and validate its canonical echo."""
    path = (
        f"/api/v1/conversations/{conversation_id}/runs/{run_id}/"
        "runner-receipt-observation/preview"
    )
    result = page.evaluate(
        """
        async ({path, accessToken, observation}) => {
          const response = await fetch(path, {
            method: 'POST',
            headers: {
              'Authorization': `Bearer ${accessToken}`,
              'Content-Type': 'application/json',
            },
            body: JSON.stringify(observation),
          });
          return {status: response.status, body: await response.text()};
        }
        """,
        {"path": path, "accessToken": access_token, "observation": observation},
    )
    if result["status"] != 200:
        raise AssertionError(
            "Forge Web session Runner receipt preview returned "
            f"{result['status']}: {result['body']}"
        )
    returned = _decode_json_response(
        result["body"], "Forge Web session Runner receipt preview"
    )
    if returned != observation:
        raise AssertionError(
            "Forge Web session Runner receipt preview response drifted: "
            f"{returned!r}"
        )
    if (
        returned.get("conversation_id") != conversation_id
        or returned.get("run_id") != run_id
        or returned.get("selected_target_id") is not None
    ):
        raise AssertionError(
            "Forge Web session Runner receipt preview lost its Run binding: "
            f"{returned!r}"
        )
    authority = returned.get("authority")
    if not isinstance(authority, dict) or any(authority.values()):
        raise AssertionError(
            "Forge Web session Runner receipt preview exposed authority: "
            f"{authority!r}"
        )
    receipt = returned.get("receipt_observation")
    if not isinstance(receipt, dict):
        raise AssertionError(
            "Forge Web session Runner receipt preview omitted terminal receipt metadata"
        )
    uncertain = receipt.get("disposition_kind") == "uncertain"
    expected = {
        "uncertain": uncertain,
        "reconciliation_required": uncertain,
        "manual_review_required": uncertain,
        "automatic_retry": False,
        "follow_up": "reconciliation_manual" if uncertain else "none",
    }
    actual = {field: receipt.get(field) for field in expected}
    if actual != expected:
        raise AssertionError(
            "Forge Web session Runner receipt reconciliation metadata drifted: "
            f"expected={expected!r} actual={actual!r}"
        )


def canonical_json_values(values: list[dict]) -> list[str]:
    return sorted(
        json.dumps(value, sort_keys=True, separators=(",", ":")) for value in values
    )


def _decode_json_response(body: str, label: str) -> dict:
    try:
        returned = json.loads(body)
    except json.JSONDecodeError as error:
        raise AssertionError(f"{label} was not JSON: {body}") from error
    if not isinstance(returned, dict):
        raise AssertionError(f"{label} returned a non-object JSON value: {returned!r}")
    return returned
