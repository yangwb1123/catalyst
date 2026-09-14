#!/usr/bin/env python3
"""Exercise the Forge Console's real Flutter Web route in Chromium."""

import json
import sys
from urllib.parse import urlsplit

from playwright.sync_api import TimeoutError as PlaywrightTimeoutError
from playwright.sync_api import sync_playwright


def main(input_path: str) -> None:
    with open(input_path, "r", encoding="utf-8") as input_file:
        test_input = json.load(input_file)

    page_url = test_input["page_url"]
    access_token = test_input["access_token"]
    conversation_id = test_input["conversation_id"]
    prompt = test_input["prompt"]
    prompt_path = f"/api/v1/conversations/{conversation_id}/prompts"
    page_errors: list[str] = []
    api_responses: list[str] = []

    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(
            headless=True,
            args=["--force-renderer-accessibility"],
        )
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
            response = page.goto(page_url, wait_until="domcontentloaded", timeout=90_000)
            if response is None or response.status != 200:
                raise AssertionError(
                    f"Forge Web route response was {None if response is None else response.status}"
                )

            accessibility_button = page.locator("flt-semantics-placeholder")
            accessibility_button.wait_for(state="attached", timeout=30_000)
            accessibility_button.evaluate("element => element.click()")

            title = "Shared from client A"
            page.get_by_text(title, exact=False).first.click(timeout=30_000)
            _wait_for_accessible_text(page, "Prompt history")

            stored_token = page.evaluate(
                "window.sessionStorage.getItem('sso_access_token:forge-console')"
            )
            if stored_token != access_token:
                raise AssertionError("Forge token was not retained in browser tab sessionStorage")

            prompt_field = page.get_by_role("textbox").last
            prompt_field.fill(prompt)
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


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: forge_console_browser_e2e.py <private-input.json>")
    main(sys.argv[1])
