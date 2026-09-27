#!/usr/bin/env python3
"""Exercise the owner-scoped Conversation SSE boundary from Chromium."""

import json
import os
import sys
from urllib.parse import urlsplit

from playwright.sync_api import sync_playwright


STREAM_PATH = "/api/v1/conversation-changes/stream"


def main(input_path: str) -> None:
    with open(input_path, "r", encoding="utf-8") as input_file:
        test_input = json.load(input_file)

    page_url = _required_text(test_input, "page_url")
    access_token = _required_text(test_input, "access_token")
    expected_page = test_input.get("expected_page")
    if not isinstance(expected_page, dict):
        raise AssertionError("expected_page must be an object")

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
                    "Forge Web Conversation SSE page response was "
                    f"{None if response is None else response.status}"
                )

            # Keep the browser journey on the same origin as the loaded Web
            # app, while making the SSE request explicit and bearer-bound.
            with page.expect_response(
                lambda candidate: candidate.request.method == "GET"
                and urlsplit(candidate.url).path == STREAM_PATH,
                timeout=90_000,
            ) as stream_response:
                actual = page.evaluate(
                    """
                    async ({accessToken, path}) => {
                      const response = await fetch(
                        `${path}?after_cursor=0&limit=1&wait_ms=0`,
                        {
                          method: "GET",
                          headers: {
                            Authorization: `Bearer ${accessToken}`,
                            Accept: "text/event-stream",
                          },
                          cache: "no-store",
                        },
                      );
                      return {
                        status: response.status,
                        contentType: response.headers.get("content-type"),
                        body: await response.text(),
                      };
                    }
                    """,
                    {"accessToken": access_token, "path": STREAM_PATH},
                )

            if actual.get("status") != 200:
                raise AssertionError(
                    "Forge Web Conversation SSE returned "
                    f"{actual.get('status')}: {actual.get('body')!r}"
                )
            content_type = actual.get("contentType")
            if not isinstance(content_type, str) or not content_type.lower().startswith(
                "text/event-stream"
            ):
                raise AssertionError(
                    f"Forge Web Conversation SSE content type was {content_type!r}"
                )
            actual_page = _parse_single_event(actual.get("body"))
            if actual_page != expected_page:
                raise AssertionError(
                    "Forge Web Conversation SSE page drifted from the canonical "
                    f"projection: expected {expected_page!r}, got {actual_page!r}"
                )
            if stream_response.value.status != 200:
                raise AssertionError(
                    "Forge Web Conversation SSE response event disagreed with "
                    f"fetch: {stream_response.value.status}"
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
                    f"Forge Web Conversation SSE page errors: {page_errors!r}"
                )
        finally:
            browser.close()


def _required_text(value: dict, key: str) -> str:
    result = value.get(key)
    if not isinstance(result, str) or not result:
        raise AssertionError(f"{key} must be a non-empty string")
    return result


def _parse_single_event(body: object) -> dict:
    if not isinstance(body, str):
        raise AssertionError(f"SSE body was not text: {body!r}")
    normalized = body.replace("\r\n", "\n")
    if "\r" in normalized or not normalized.endswith("\n\n"):
        raise AssertionError(f"Malformed Conversation SSE framing: {body!r}")
    frame = normalized[:-2]
    lines = frame.split("\n")
    if not lines or any(not line for line in lines):
        raise AssertionError(f"Malformed Conversation SSE frame: {body!r}")
    fields: dict[str, str] = {}
    for line in lines:
        separator = line.find(":")
        if separator <= 0:
            raise AssertionError(f"Malformed Conversation SSE field: {line!r}")
        field = line[:separator]
        value = line[separator + 1 :]
        if value.startswith(" "):
            value = value[1:]
        if field not in {"event", "id", "data"} or field in fields:
            raise AssertionError(f"Unexpected or duplicate Conversation SSE field: {line!r}")
        fields[field] = value
    if set(fields) != {"event", "id", "data"}:
        raise AssertionError(f"Incomplete Conversation SSE frame: {fields!r}")
    if fields["event"] != "conversation_changes":
        raise AssertionError(f"Unexpected Conversation SSE event: {fields['event']!r}")
    if not fields["id"].isdigit() or str(int(fields["id"])) != fields["id"]:
        raise AssertionError(f"Unsafe Conversation SSE cursor: {fields['id']!r}")
    try:
        payload = json.loads(fields["data"])
    except json.JSONDecodeError as error:
        raise AssertionError(f"Conversation SSE data was not JSON: {fields['data']!r}") from error
    if not isinstance(payload, dict):
        raise AssertionError(f"Conversation SSE data was not an object: {payload!r}")
    if payload.get("scanned_through_cursor") != int(fields["id"]):
        raise AssertionError(
            "Conversation SSE event cursor did not match the dense page: "
            f"id={fields['id']!r} page={payload!r}"
        )
    return payload


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit(f"usage: {sys.argv[0]} INPUT_JSON")
    main(sys.argv[1])
