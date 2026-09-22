#!/usr/bin/env python3
"""Exercise the Forge Console's real Flutter Web route in Chromium."""

import json
import os
import sys
from urllib.parse import parse_qs, urlsplit

from playwright.sync_api import TimeoutError as PlaywrightTimeoutError
from playwright.sync_api import sync_playwright

from forge_console_browser_e2e_helpers import (
    post_session_device_observation,
    post_session_runner_receipt_observation,
)


def main(input_path: str) -> None:
    with open(input_path, "r", encoding="utf-8") as input_file:
        test_input = json.load(input_file)

    page_url = test_input["page_url"]
    access_token = test_input["access_token"]
    conversation_id = test_input["conversation_id"]
    prompt = test_input["prompt"]
    conversation_title = test_input.get("conversation_title", "Shared from client A")
    run_id = test_input.get("run_id")
    preflight_request = test_input.get("run_attempt_lease_dispatch_preflight_request")
    preflight_expected = test_input.get("run_attempt_lease_dispatch_preflight_expected")
    reload_session = bool(test_input.get("reload_session", False))
    deep_link = bool(test_input.get("deep_link", False))
    prompt_path = f"/api/v1/conversations/{conversation_id}/prompts"
    page_errors: list[str] = []
    api_responses: list[str] = []

    with sync_playwright() as playwright:
        launch_options = {
            "headless": True,
            "args": ["--force-renderer-accessibility"],
        }
        executable_path = os.environ.get("FORGE_BROWSER_EXECUTABLE")
        if executable_path:
            launch_options["executable_path"] = executable_path
        browser = playwright.chromium.launch(**launch_options)
        try:
            context = browser.new_context(viewport={"width": 1440, "height": 1400})
            token_key = json.dumps("sso_access_token:forge-console")
            token_value = json.dumps(access_token)
            context.add_init_script(
                f"window.sessionStorage.setItem({token_key}, {token_value});"
            )
            page = context.new_page()
            page.on("pageerror", lambda error: page_errors.append(str(error)))
            page.on(
                "response",
                lambda response: api_responses.append(
                    f"{response.request.method} {urlsplit(response.url).path} {response.status}"
                )
                if urlsplit(response.url).path.startswith("/api/v1/")
                else None,
            )
            response = page.goto(
                page_url, wait_until="domcontentloaded", timeout=90_000
            )
            if response is None or response.status != 200:
                raise AssertionError(
                    f"Forge Web route response was {None if response is None else response.status}"
                )
            if deep_link:
                _assert_deep_link_path(page, conversation_id)

            accessibility_button = page.locator("flt-semantics-placeholder")
            accessibility_button.wait_for(state="attached", timeout=30_000)
            if reload_session and not run_id:
                # Flutter Web starts the authenticated route after its
                # semantics bridge is enabled. Capture the first feed around
                # that click so the test proves the actual bootstrap request.
                with page.expect_response(
                    _is_change_feed_response,
                    timeout=90_000,
                ) as initial_change_response:
                    accessibility_button.evaluate("element => element.click()")
                initial_after_cursor = _change_feed_cursor(
                    initial_change_response.value
                )
                if initial_after_cursor != 0:
                    raise AssertionError(
                        "Forge Web change-feed bootstrap did not start at cursor 0: "
                        f"{initial_after_cursor}"
                    )
            else:
                accessibility_button.evaluate("element => element.click()")

            title = conversation_title
            if deep_link:
                _wait_for_accessible_text(page, "Prompt history")
            else:
                page.get_by_text(conversation_title, exact=False).first.click(timeout=30_000)
            _wait_for_accessible_text(page, "Prompt history")

            stored_token = page.evaluate(
                "window.sessionStorage.getItem('sso_access_token:forge-console')"
            )
            if stored_token != access_token:
                raise AssertionError("Forge token was not retained in browser tab sessionStorage")

            if reload_session and not run_id:
                _assert_change_feed_cursor_resumes_after_reload(
                    page, title, conversation_id, deep_link
                )

            if run_id:
                _wait_for_accessible_text(page, run_id)
                run_observed = test_input.get("run_observed")
                if run_observed is not None:
                    # The ordinary Forge entry point intentionally leaves the
                    # candidate reader unset. The opt-in observation journey
                    # exercises the same authenticated browser transport
                    # explicitly, without changing the production Gate.
                    observed_response = page.evaluate(
                        """
                        async ({conversationID, runID, accessToken}) => {
                          const path = `/api/v1/conversations/${encodeURIComponent(conversationID)}/runs/${encodeURIComponent(runID)}/observation`;
                          const response = await fetch(path, {
                            method: "GET",
                            headers: {Authorization: `Bearer ${accessToken}`},
                            cache: "no-store",
                          });
                          return {status: response.status, body: await response.text()};
                        }
                        """,
                        {
                            "conversationID": conversation_id,
                            "runID": run_id,
                            "accessToken": access_token,
                        },
                    )
                    if observed_response.get("status") != 200:
                        raise AssertionError(
                            "Forge Web Run observation returned "
                            f"{observed_response.get('status')}: {observed_response.get('body')!r}"
                        )
                    try:
                        actual_run_observed = json.loads(observed_response["body"])
                    except (KeyError, TypeError, json.JSONDecodeError) as error:
                        raise AssertionError(
                            "Forge Web Run observation was not valid JSON: "
                            f"{observed_response!r}"
                        ) from error
                    if actual_run_observed != run_observed:
                        raise AssertionError(
                            "Forge Web Run observation drifted from the canonical "
                            f"projection: expected {run_observed!r}, got {actual_run_observed!r}"
                        )
                    _assert_run_observed_display_only(
                        actual_run_observed, conversation_id, run_id
                    )
                page.get_by_text(run_id, exact=False).first.click(timeout=30_000)
                _wait_for_accessible_text(page, f"Run timeline · {run_id}")
                _wait_for_visible_text(page, "run_started")
                _wait_for_visible_text(page, "run_finished")
                # Reload the real Web route with the same tab credential and
                # let the persisted owner-bound Run checkpoint drive the
                # second timeline read. The fresh route must not replay the
                # first page's markers.
                page.reload(wait_until="domcontentloaded", timeout=90_000)
                accessibility_button = page.locator("flt-semantics-placeholder")
                accessibility_button.wait_for(state="attached", timeout=30_000)
                accessibility_button.evaluate("element => element.click()")
                page.get_by_text(title, exact=False).first.click(timeout=30_000)
                _wait_for_accessible_text(page, "Prompt history")
                _wait_for_accessible_text(page, run_id)
                page.get_by_label(run_id, exact=False).first.click(timeout=30_000)
                _wait_for_accessible_text(page, f"Run timeline · {run_id}")
                _wait_for_accessible_text(page, "No timeline markers to show.")
                if page.get_by_text("run_started", exact=False).count() or page.get_by_text(
                    "run_finished", exact=False
                ).count():
                    raise AssertionError("Forge Web remount replayed Run timeline markers")
                session_observation = test_input.get("session_observation")
                if session_observation is not None:
                    session_observation = post_session_device_observation(
                        page,
                        access_token,
                        conversation_id,
                        run_id,
                        session_observation,
                    )
                    page.get_by_role(
                        "button",
                        name="Import offline device observation",
                        exact=True,
                    ).click(timeout=30_000)
                    observation_textbox = page.get_by_role(
                        "textbox", name="Canonical observation JSON", exact=True
                    )
                    observation_textbox.wait_for(state="visible", timeout=30_000)
                    mutated_session_observation = json.loads(
                        json.dumps(session_observation, separators=(",", ":"))
                    )
                    mutated_session_observation["selected_device_id"] = "candidate-a"
                    observation_textbox.fill(
                        json.dumps(mutated_session_observation, separators=(",", ":"))
                    )
                    page.get_by_role("button", name="Import", exact=True).last.click(
                        timeout=30_000
                    )
                    _wait_for_visible_text(
                        page, "Invalid offline device observation."
                    )
                    page.get_by_role("button", name="Cancel", exact=True).click(
                        timeout=30_000
                    )
                    page.get_by_role(
                        "button",
                        name="Import offline device observation",
                        exact=True,
                    ).click(timeout=30_000)
                    observation_textbox = page.get_by_role(
                        "textbox", name="Canonical observation JSON", exact=True
                    )
                    observation_textbox.wait_for(state="visible", timeout=30_000)
                    observation_textbox.fill(
                        json.dumps(session_observation, separators=(",", ":"))
                    )
                    page.get_by_role("button", name="Import", exact=True).last.click(
                        timeout=30_000
                    )
                    _wait_for_visible_text(page, "Devices: 9 · Runner instances: 9")
                    _wait_for_visible_text(
                        page, "Eligible declared devices: 2 · instances: 2"
                    )
                    # The summary is above the lazy device list. Resolve the
                    # first row before moving toward the tail; a fixed large
                    # wheel delta can virtualize the first rows away on
                    # different browser viewport sizes.
                    _wait_for_visible_text(page, "Device: candidate-a")
                    page.mouse.wheel(0, 5000)
                    page.wait_for_timeout(500)
                    for _ in range(12):
                        if page.get_by_text("Device: candidate-i", exact=False).count():
                            break
                        page.mouse.wheel(0, 1200)
                        page.wait_for_timeout(250)
                    _wait_for_visible_text(page, "Device: candidate-i")
                runner_execution_observation = test_input.get(
                    "runner_execution_observation"
                )
                if runner_execution_observation is not None:
                    page.mouse.wheel(0, -5000)
                    page.wait_for_timeout(500)
                    page.get_by_role(
                        "button",
                        name="Import Runner execution observation",
                        exact=True,
                    ).click(timeout=30_000)
                    runner_observation_textbox = page.get_by_role(
                        "textbox",
                        name="Canonical Runner execution observation JSON",
                        exact=True,
                    )
                    runner_observation_textbox.wait_for(
                        state="visible", timeout=30_000
                    )
                    runner_observation_textbox.fill(
                        json.dumps(
                            runner_execution_observation, separators=(",", ":")
                        )
                    )
                    page.get_by_role("button", name="Import", exact=True).last.click(
                        timeout=30_000
                    )
                    page.mouse.wheel(0, 5000)
                    page.wait_for_timeout(500)
                    _wait_for_visible_text(page, "Runner execution intent preview")
                    _wait_for_visible_text(page, "attempt-1")
                    _wait_for_visible_text(page, "command-1")
                    _wait_for_visible_text(page, "runner-1")
                session_runner_receipt_observation = test_input.get(
                    "session_runner_receipt_observation"
                )
                if session_runner_receipt_observation is not None:
                    page.mouse.wheel(0, -5000)
                    page.wait_for_timeout(500)
                    post_session_runner_receipt_observation(
                        page,
                        access_token,
                        conversation_id,
                        run_id,
                        session_runner_receipt_observation,
                    )
                    page.get_by_role(
                        "button",
                        name="Import session Runner receipt observation",
                        exact=True,
                    ).click(timeout=30_000)
                    receipt_observation_textbox = page.get_by_role(
                        "textbox",
                        name="Canonical session Runner receipt observation JSON",
                        exact=True,
                    )
                    receipt_observation_textbox.wait_for(
                        state="visible", timeout=30_000
                    )
                    # Prove that the real Web import boundary rejects a
                    # caller mutation before the canonical value is shown.
                    # This remains local-only: the recorder below still
                    # requires exactly one authenticated preview POST.
                    mutated_receipt_observation = json.loads(
                        json.dumps(
                            session_runner_receipt_observation,
                            separators=(",", ":"),
                        )
                    )
                    mutated_receipt_observation["selected_target_id"] = "runner-1"
                    receipt_observation_textbox.fill(
                        json.dumps(
                            mutated_receipt_observation,
                            separators=(",", ":"),
                        )
                    )
                    page.get_by_role("button", name="Import", exact=True).last.click(
                        timeout=30_000
                    )
                    _wait_for_visible_text(
                        page,
                        "Invalid or foreign session Runner receipt observation.",
                    )
                    # Reopen the dialog to ensure the rejected value cannot be
                    # reused accidentally by a later successful import.
                    page.get_by_role("button", name="Cancel", exact=True).click(
                        timeout=30_000
                    )
                    page.get_by_role(
                        "button",
                        name="Import session Runner receipt observation",
                        exact=True,
                    ).click(timeout=30_000)
                    receipt_observation_textbox = page.get_by_role(
                        "textbox",
                        name="Canonical session Runner receipt observation JSON",
                        exact=True,
                    )
                    receipt_observation_textbox.wait_for(
                        state="visible", timeout=30_000
                    )
                    receipt_observation_textbox.fill(
                        json.dumps(
                            session_runner_receipt_observation,
                            separators=(",", ":"),
                        )
                    )
                    page.get_by_role("button", name="Import", exact=True).last.click(
                        timeout=30_000
                    )
                    # The receipt card follows the bounded inventory cards in
                    # the lazy Web ListView. Scroll in finite increments until
                    # it is materialized instead of assuming one wheel delta
                    # reaches the end of the list.
                    for _ in range(24):
                        if page.get_by_text(
                            "Session Runner receipt preview", exact=False
                        ).count():
                            break
                        page.mouse.wheel(0, 1200)
                        page.wait_for_timeout(150)
                    _wait_for_visible_text(page, "Session Runner receipt preview")
                    _wait_for_visible_text(page, "attempt-1")
                    _wait_for_visible_text(page, "command-1")
                    _wait_for_visible_text(page, "runner-1")
                    receipt = session_runner_receipt_observation.get(
                        "receipt_observation", {}
                    )
                    _wait_for_visible_text(
                        page, receipt.get("disposition_kind", "")
                    )
                    _wait_for_visible_text(page, receipt.get("follow_up", ""))
                body_text = page.locator("body").inner_text(timeout=5_000)
                if "deterministic Run observation fixture" in body_text or "private event payload" in body_text:
                    raise AssertionError("Forge Web Run timeline exposed payload text")
            elif preflight_request is None:
                prompt_field = page.get_by_role(
                    "textbox", name="Enter a prompt to send.", exact=True
                )
                if not prompt_field.count():
                    prompt_field = page.get_by_role("textbox").last
                # Flutter Web's semantics textbox is backed by a hidden
                # platform input. Focus it before inserting so the framework
                # receives the edit event and enables Append prompt.
                prompt_field.click(timeout=30_000)
                page.keyboard.press("Control+A")
                page.keyboard.insert_text(prompt)
                with page.expect_response(
                    lambda response: response.request.method == "POST"
                    and urlsplit(response.url).path == prompt_path,
                    timeout=30_000,
                ) as prompt_post:
                    page.get_by_role("button", name="Append prompt", exact=True).click()
                post_response = prompt_post.value
                if post_response.status != 201:
                    raise AssertionError(f"Forge Prompt submission returned {post_response.status}")
                page.wait_for_timeout(250)

            if preflight_request is not None:
                if not isinstance(preflight_request, dict) or not isinstance(
                    preflight_expected, dict
                ):
                    raise AssertionError(
                        "Forge Web Run-Attempt lease preflight input must be objects"
                    )
                preflight_run_id = preflight_request.get("run_id")
                if not isinstance(preflight_run_id, str) or not preflight_run_id:
                    raise AssertionError(
                        "Forge Web Run-Attempt lease preflight request must include run_id"
                    )
                preflight_response = page.evaluate(
                    """
                    async ({conversationID, runID, accessToken, request}) => {
                      const path = `/api/v1/conversations/${encodeURIComponent(conversationID)}/runs/${encodeURIComponent(runID)}/attempt-lease-dispatch-preflight/preview`;
                      const response = await fetch(path, {
                        method: "POST",
                        headers: {
                          Authorization: `Bearer ${accessToken}`,
                          "Content-Type": "application/json",
                        },
                        body: JSON.stringify(request),
                        cache: "no-store",
                      });
                      return {status: response.status, body: await response.text()};
                    }
                    """,
                    {
                        "conversationID": conversation_id,
                        "runID": preflight_run_id,
                        "accessToken": access_token,
                        "request": preflight_request,
                    },
                )
                if preflight_response.get("status") != 200:
                    raise AssertionError(
                        "Forge Web Run-Attempt lease preflight returned "
                        f"{preflight_response.get('status')}: {preflight_response.get('body')!r}"
                    )
                try:
                    actual_preflight = json.loads(preflight_response["body"])
                except (KeyError, TypeError, json.JSONDecodeError) as error:
                    raise AssertionError(
                        "Forge Web Run-Attempt lease preflight was not valid JSON: "
                        f"{preflight_response!r}"
                    ) from error
                if actual_preflight != preflight_expected:
                    raise AssertionError(
                        "Forge Web Run-Attempt lease preflight drifted from the canonical "
                        f"projection: expected {preflight_expected!r}, got {actual_preflight!r}"
                    )
                _assert_run_attempt_lease_dispatch_preflight_display_only(
                    actual_preflight, conversation_id, preflight_run_id
                )

            if page_errors:
                raise AssertionError("Forge Web raised browser errors: " + "; ".join(page_errors))
        except PlaywrightTimeoutError as error:
            body_text = page.locator("body").inner_text(timeout=5_000)
            accessible_labels = page.locator("flt-semantics[aria-label]").evaluate_all(
                "nodes => nodes.map(node => node.getAttribute('aria-label')).slice(0, 80)"
            )
            raise AssertionError(
                "Timed out during Forge Web interaction; "
                f"visible page text={body_text!r}; "
                f"accessible labels={accessible_labels!r}; API responses={api_responses!r}"
                f"; page errors={page_errors!r}"
            ) from error
        finally:
            browser.close()


def _wait_for_accessible_text(page, expected_text: str) -> None:
    page.wait_for_function(
        "expected => Array.from(document.querySelectorAll('flt-semantics[aria-label]'))"
        ".some(node => (node.getAttribute('aria-label') || '').includes(expected))",
        arg=expected_text,
        timeout=30_000,
    )


def _wait_for_visible_text(page, expected_text: str) -> None:
    deadline_ms = 30_000
    elapsed_ms = 0
    while elapsed_ms < deadline_ms:
        locator = page.get_by_text(expected_text, exact=False).first
        if locator.count():
            try:
                locator.scroll_into_view_if_needed(timeout=1_000)
                if locator.is_visible():
                    return
            except PlaywrightTimeoutError:
                pass
            except Exception:
                # Flutter Web may replace a semantics node while a dialog
                # closes or a lazy list materializes; reacquire it next pass.
                pass
        try:
            if expected_text in page.locator("body").inner_text(timeout=1_000):
                return
        except Exception:
            pass
        page.mouse.wheel(0, 1_000)
        page.wait_for_timeout(250)
        elapsed_ms += 250
    page.get_by_text(expected_text, exact=False).first.wait_for(
        state="visible", timeout=1_000
    )


def _assert_run_observed_display_only(
    observation: dict, conversation_id: str, run_id: str
) -> None:
    """Keep the browser candidate bound and content-free."""
    expected_keys = {
        "api_version",
        "owner_ref",
        "conversation_id",
        "run_id",
        "prompt_id",
        "created_at_ms",
        "latest_sequence",
        "status",
        "metadata_observed",
        "content_included",
        "authority",
    }
    if not isinstance(observation, dict) or set(observation) != expected_keys:
        raise AssertionError(
            f"Forge Web Run observation had an open or incomplete shape: {observation!r}"
        )
    if (
        observation["api_version"] != "forge.run.observed.v1"
        or observation["conversation_id"] != conversation_id
        or observation["run_id"] != run_id
        or not observation["owner_ref"]
        or observation["metadata_observed"] is not True
        or observation["content_included"] is not False
    ):
        raise AssertionError(
            f"Forge Web Run observation lost its binding/display boundary: {observation!r}"
        )
    authority = observation["authority"]
    expected_authority_keys = {
        "identity_verified",
        "owner_authorized",
        "run_authoritative",
        "persistence_attested",
        "content_provenance_verified",
        "reservation_created",
        "execution_authorized",
        "dispatch_performed",
    }
    if (
        not isinstance(authority, dict)
        or set(authority) != expected_authority_keys
        or any(value is not False for value in authority.values())
    ):
        raise AssertionError(
            f"Forge Web Run observation exposed authority: {observation!r}"
        )


def _assert_run_attempt_lease_dispatch_preflight_display_only(
    observation: dict, conversation_id: str, run_id: str
) -> None:
    """Keep the browser preflight candidate metadata-only and path-bound."""
    expected_keys = {
        "schema_version",
        "evaluation_mode",
        "owner",
        "conversation_id",
        "run_id",
        "run_status",
        "run_state_admissible",
        "attempt_id",
        "attempt_state",
        "attempt_state_admissible",
        "command_id",
        "intent_target_id",
        "lease_epoch",
        "lease_active",
        "evaluated_at_ms",
        "candidate_count",
        "declarative_ready_count",
        "declarative_preflight_ready",
        "rejection_reasons",
        "selected_target_id",
        "preview_only",
        "authority",
    }
    if not isinstance(observation, dict) or set(observation) != expected_keys:
        raise AssertionError(
            "Forge Web Run-Attempt lease preflight had an open or incomplete shape: "
            f"{observation!r}"
        )
    if (
        observation["schema_version"]
        != "forge.run-attempt-lease-dispatch-preflight/v1"
        or observation["evaluation_mode"] != "pure_run_attempt_lease_dispatch_preflight"
        or observation["conversation_id"] != conversation_id
        or observation["run_id"] != run_id
        or observation["selected_target_id"] is not None
        or observation["declarative_preflight_ready"] is not True
        or observation["rejection_reasons"] != []
        or observation["preview_only"] is not True
    ):
        raise AssertionError(
            "Forge Web Run-Attempt lease preflight lost its binding/display boundary: "
            f"{observation!r}"
        )
    authority = observation["authority"]
    if not isinstance(authority, dict) or set(authority) != {
        "identity_verified",
        "run_authoritative",
        "attempt_persisted",
        "lease_issued",
        "reservation_created",
        "execution_authorized",
        "dispatch_performed",
        "audit_published",
    } or any(value is not False for value in authority.values()):
        raise AssertionError(
            f"Forge Web Run-Attempt lease preflight exposed authority: {observation!r}"
        )


def _assert_change_feed_cursor_resumes_after_reload(
    page, conversation_title: str, conversation_id: str, deep_link: bool
) -> None:
    """Prove the real Web route persists and resumes its owner change cursor."""
    page.wait_for_function(
        "prefix => Object.keys(window.localStorage).some(key => {"
        "  if (!key.startsWith(prefix)) return false;"
        "  try { return Number(JSON.parse(window.localStorage.getItem(key)).cursor) > 0; }"
        "  catch (_) { return false; }"
        "})",
        arg="forge_conversation_changes_v1:",
        timeout=30_000,
    )

    with page.expect_response(
        _is_change_feed_response,
        timeout=90_000,
    ) as resumed_change_response:
        page.reload(wait_until="domcontentloaded", timeout=90_000)
        accessibility_button = page.locator("flt-semantics-placeholder")
        accessibility_button.wait_for(state="attached", timeout=30_000)
        accessibility_button.evaluate("element => element.click()")
    resumed_after_cursor = _change_feed_cursor(resumed_change_response.value)
    if resumed_after_cursor <= 0:
        raise AssertionError(
            "Forge Web reload replayed the initial change cursor: "
            f"{resumed_after_cursor}"
        )
    if deep_link:
        _assert_deep_link_path(page, conversation_id)
    else:
        page.get_by_text(conversation_title, exact=False).first.click(timeout=30_000)
    _wait_for_accessible_text(page, "Prompt history")


def _assert_deep_link_path(page, conversation_id: str) -> None:
    expected_path = f"/forge/conversations/{conversation_id}"
    actual_path = urlsplit(page.url).path
    if actual_path != expected_path:
        raise AssertionError(
            f"Forge Web deep link changed unexpectedly: expected {expected_path!r}, got {actual_path!r}"
        )


def _is_change_feed_response(response) -> bool:
    return (
        response.request.method == "GET"
        and urlsplit(response.url).path == "/api/v1/conversation-changes"
    )


def _change_feed_cursor(response) -> int:
    """Return the exact cursor used by a change-feed GET response."""
    if response.status != 200:
        raise AssertionError(
            f"Forge Web change-feed request returned {response.status}"
        )
    query = parse_qs(urlsplit(response.url).query)
    values = query.get("after_cursor")
    if values is None or len(values) != 1:
        raise AssertionError(
            f"Forge Web change-feed request omitted after_cursor: {response.url}"
        )
    try:
        return int(values[0])
    except ValueError as error:
        raise AssertionError(
            f"Forge Web change-feed cursor was invalid: {response.url}"
        ) from error


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: forge_console_browser_e2e.py <private-input.json>")
    main(sys.argv[1])
