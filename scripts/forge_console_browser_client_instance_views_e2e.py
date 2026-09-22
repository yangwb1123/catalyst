#!/usr/bin/env python3
"""Exercise the opt-in Forge Web client-instance view candidates in Chromium."""

import json
import os
import sys
from urllib.parse import urlsplit

from playwright.sync_api import sync_playwright


SESSION_VIEW_PATH = "/api/v1/client-instances/session-view"
RESOURCE_VIEW_PATH = "/api/v1/client-instances/resource-view"


def main(input_path: str) -> None:
    with open(input_path, "r", encoding="utf-8") as input_file:
        test_input = json.load(input_file)

    page_url = test_input["page_url"]
    access_token = test_input["access_token"]
    expected_owner = test_input["owner"]
    expected_session_view = test_input["session_view"]
    expected_resource_view = test_input["resource_view"]
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
                    "Forge Web client-instance route response was "
                    f"{None if response is None else response.status}"
                )

            accessibility_button = page.locator("flt-semantics-placeholder")
            accessibility_button.wait_for(state="attached", timeout=30_000)
            accessibility_button.evaluate("element => element.click()")
            page.wait_for_timeout(500)

            actual_session_view = _fetch_candidate(
                page, access_token, SESSION_VIEW_PATH
            )
            if actual_session_view != expected_session_view:
                raise AssertionError(
                    "Forge Web client-instance/session-view drifted from the canonical "
                    f"projection: expected {expected_session_view!r}, got {actual_session_view!r}"
                )
            _assert_session_view_display_only(actual_session_view, expected_owner)

            actual_resource_view = _fetch_candidate(
                page, access_token, RESOURCE_VIEW_PATH
            )
            if actual_resource_view != expected_resource_view:
                raise AssertionError(
                    "Forge Web client-instance/resource-view drifted from the canonical "
                    f"projection: expected {expected_resource_view!r}, got {actual_resource_view!r}"
                )
            _assert_resource_view_display_only(actual_resource_view, expected_owner)

            for path in (SESSION_VIEW_PATH, RESOURCE_VIEW_PATH):
                candidate_responses = [
                    item
                    for item in api_responses
                    if item.endswith(f"{path} 200")
                ]
                if len(candidate_responses) != 1:
                    raise AssertionError(
                        "Forge Web client-instance candidate request count drifted: "
                        f"path={path!r} responses={api_responses!r}"
                    )

            effect_paths = (
                "/api/v1/devices",
                "/api/v1/device-placement",
                "/api/v1/runners",
                "/api/v1/dispatch",
            )
            effect_responses = [
                item for item in api_responses if any(path in item for path in effect_paths)
            ]
            if effect_responses:
                raise AssertionError(
                    "Forge Web client-instance journey contacted an execution/device route: "
                    f"{effect_responses!r}"
                )

            stored_token = page.evaluate(
                "window.sessionStorage.getItem('sso_access_token:forge-console')"
            )
            if stored_token != access_token:
                raise AssertionError(
                    "Forge token was not retained in browser tab sessionStorage"
                )
            if page_errors:
                raise AssertionError(
                    f"Forge Web client-instance page errors: {page_errors!r}"
                )
        finally:
            browser.close()


def _fetch_candidate(page, access_token: str, path: str) -> dict:
    with page.expect_response(
        lambda candidate: candidate.request.method == "GET"
        and urlsplit(candidate.url).path == path,
        timeout=90_000,
    ) as candidate_response:
        actual_response = page.evaluate(
            """
            async ({accessToken, path}) => {
              const response = await fetch(path, {
                method: "GET",
                headers: {Authorization: `Bearer ${accessToken}`},
                cache: "no-store",
              });
              return {status: response.status, body: await response.text()};
            }
            """,
            {"accessToken": access_token, "path": path},
        )

    if actual_response.get("status") != 200:
        raise AssertionError(
            f"Forge Web client-instance candidate {path} returned "
            f"{actual_response.get('status')}: {actual_response.get('body')!r}"
        )
    if candidate_response.value.status != 200:
        raise AssertionError(
            f"Forge Web client-instance candidate response event disagreed for {path}: "
            f"{candidate_response.value.status}"
        )
    try:
        return json.loads(actual_response["body"])
    except (KeyError, TypeError, json.JSONDecodeError) as error:
        raise AssertionError(
            f"Forge Web client-instance candidate {path} was not valid JSON: "
            f"{actual_response!r}"
        ) from error


def _assert_session_view_display_only(view: dict, owner: dict) -> None:
    expected_keys = {
        "schema_version",
        "evaluation_mode",
        "owner_declaration",
        "owner_declaration_unverified",
        "instances",
        "read_only",
        "authority",
    }
    if not isinstance(view, dict) or set(view) != expected_keys:
        raise AssertionError(f"Forge Web session view had an open shape: {view!r}")
    if (
        view["schema_version"] != "forge.client-instance-session-view/v1"
        or view["evaluation_mode"] != "owner_bound_session_view_only"
        or view["owner_declaration"] != owner
        or view["owner_declaration_unverified"] is not True
        or view["read_only"] is not True
        or not isinstance(view["instances"], list)
        or not isinstance(view["authority"], dict)
        or any(view["authority"].values())
    ):
        raise AssertionError(f"Forge Web session view crossed its display boundary: {view!r}")


def _assert_resource_view_display_only(view: dict, owner: dict) -> None:
    expected_keys = {
        "schema_version",
        "evaluation_mode",
        "owner_declaration",
        "owner_declaration_unverified",
        "instances",
        "devices",
        "device_attributes_unverified",
        "read_only",
        "authority",
    }
    if not isinstance(view, dict) or set(view) != expected_keys:
        raise AssertionError(f"Forge Web resource view had an open shape: {view!r}")
    if (
        view["schema_version"] != "forge.client-instance-resource-view/v1"
        or view["evaluation_mode"] != "owner_bound_instance_resource_view_only"
        or view["owner_declaration"] != owner
        or view["owner_declaration_unverified"] is not True
        or view["device_attributes_unverified"] is not True
        or view["read_only"] is not True
        or not isinstance(view["instances"], list)
        or not isinstance(view["devices"], list)
        or not isinstance(view["authority"], dict)
        or any(view["authority"].values())
    ):
        raise AssertionError(f"Forge Web resource view crossed its display boundary: {view!r}")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit(f"usage: {sys.argv[0]} INPUT_JSON")
    main(sys.argv[1])
