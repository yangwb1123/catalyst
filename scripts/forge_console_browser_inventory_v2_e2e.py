#!/usr/bin/env python3
"""Exercise the opt-in Forge Web v2 inventory candidate in Chromium."""

import json
import os
import sys
from urllib.parse import urlsplit

from playwright.sync_api import sync_playwright


INVENTORY_PATH = "/api/v1/devices/observations/v2"


def main(input_path: str) -> None:
    with open(input_path, "r", encoding="utf-8") as input_file:
        test_input = json.load(input_file)

    page_url = test_input["page_url"]
    access_token = test_input["access_token"]
    expected_inventory = test_input["inventory"]
    expected_owner = test_input["owner"]
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
                    f"Forge Web inventory route response was {None if response is None else response.status}"
                )

            accessibility_button = page.locator("flt-semantics-placeholder")
            accessibility_button.wait_for(state="attached", timeout=30_000)
            accessibility_button.evaluate("element => element.click()")
            page.wait_for_timeout(500)

            with page.expect_response(
                lambda candidate: candidate.request.method == "GET"
                and urlsplit(candidate.url).path == INVENTORY_PATH,
                timeout=90_000,
            ) as inventory_response:
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
                    {"accessToken": access_token, "path": INVENTORY_PATH},
                )

            if actual_response.get("status") != 200:
                raise AssertionError(
                    "Forge Web v2 inventory returned "
                    f"{actual_response.get('status')}: {actual_response.get('body')!r}"
                )
            try:
                actual_inventory = json.loads(actual_response["body"])
            except (KeyError, TypeError, json.JSONDecodeError) as error:
                raise AssertionError(
                    f"Forge Web v2 inventory was not valid JSON: {actual_response!r}"
                ) from error
            if actual_inventory != expected_inventory:
                raise AssertionError(
                    "Forge Web v2 inventory drifted from the canonical projection: "
                    f"expected {expected_inventory!r}, got {actual_inventory!r}"
                )
            _assert_inventory_display_only(actual_inventory, expected_owner)

            if inventory_response.value.status != 200:
                raise AssertionError(
                    "Forge Web v2 inventory response event disagreed with fetch: "
                    f"{inventory_response.value.status}"
                )
            candidate_responses = [
                response
                for response in api_responses
                if response.endswith(f"{INVENTORY_PATH} 200")
            ]
            if len(candidate_responses) != 1:
                raise AssertionError(
                    "Forge Web v2 inventory candidate request count drifted: "
                    f"{api_responses!r}"
                )
            stored_token = page.evaluate(
                "window.sessionStorage.getItem('sso_access_token:forge-console')"
            )
            if stored_token != access_token:
                raise AssertionError(
                    "Forge token was not retained in browser tab sessionStorage"
                )
            if page_errors:
                raise AssertionError(f"Forge Web inventory page errors: {page_errors!r}")
        finally:
            browser.close()


def _assert_inventory_display_only(inventory: dict, owner: dict) -> None:
    expected_keys = {
        "schema_version",
        "evaluation_mode",
        "evaluated_at_ms",
        "owner_declaration",
        "owner_declaration_unverified",
        "inventory_declarations_unverified",
        "notice",
        "devices",
        "execution_authorized",
        "reservation_created",
        "dispatch_performed",
    }
    if not isinstance(inventory, dict) or set(inventory) != expected_keys:
        raise AssertionError(
            f"Forge Web v2 inventory had an open or incomplete shape: {inventory!r}"
        )
    if (
        inventory["schema_version"] != "forge.device-inventory-observation/v2"
        or inventory["evaluation_mode"] != "offline_static_only"
        or inventory["owner_declaration"] != owner
        or inventory["owner_declaration_unverified"] is not True
        or inventory["inventory_declarations_unverified"] is not True
        or inventory["execution_authorized"] is not False
        or inventory["reservation_created"] is not False
        or inventory["dispatch_performed"] is not False
        or not isinstance(inventory["devices"], list)
    ):
        raise AssertionError(
            f"Forge Web v2 inventory crossed its display-only boundary: {inventory!r}"
        )


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit(f"usage: {sys.argv[0]} INPUT_JSON")
    main(sys.argv[1])
