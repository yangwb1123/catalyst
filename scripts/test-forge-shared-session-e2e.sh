#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
SNAPLINK_CONSOLE_ROOT="${SNAPLINK_CONSOLE_ROOT:-$(dirname -- "$REPO_ROOT")/workspace/demo/snaplink-console}"
FLUTTER_BIN="${FLUTTER_BIN:-flutter}"
FORGE_RUNTIME_BIN="${FORGE_RUNTIME_BIN:-$REPO_ROOT/forge-runtime/target/debug/forge-runtime}"
FORGE_BROWSER_E2E="${FORGE_BROWSER_E2E:-0}"
FORGE_CLIENT_INSTANCE_SESSION_PROJECTION_E2E="${FORGE_CLIENT_INSTANCE_SESSION_PROJECTION_E2E:-1}"
FORGE_CLIENT_INSTANCE_RESOURCE_PROJECTION_E2E="${FORGE_CLIENT_INSTANCE_RESOURCE_PROJECTION_E2E:-1}"
FORGE_CLIENT_INSTANCE_DISPATCH_PLAN_PROJECTION_E2E="${FORGE_CLIENT_INSTANCE_DISPATCH_PLAN_PROJECTION_E2E:-1}"
FORGE_CLIENT_INSTANCE_EXECUTION_READINESS_PROJECTION_E2E="${FORGE_CLIENT_INSTANCE_EXECUTION_READINESS_PROJECTION_E2E:-1}"
FORGE_CLIENT_INSTANCE_EXECUTION_RECONCILIATION_PROJECTION_E2E="${FORGE_CLIENT_INSTANCE_EXECUTION_RECONCILIATION_PROJECTION_E2E:-1}"
FORGE_BROWSER_PYTHON="${FORGE_BROWSER_PYTHON:-python3}"

if [[ ! -f "$SNAPLINK_CONSOLE_ROOT/pubspec.yaml" ]]; then
  printf 'Flutter Console repo not found: %s\n' "$SNAPLINK_CONSOLE_ROOT" >&2
  exit 1
fi
if ! command -v "$FLUTTER_BIN" >/dev/null 2>&1; then
  printf 'Flutter executable not found: %s\n' "$FLUTTER_BIN" >&2
  exit 1
fi
if ! command -v script >/dev/null 2>&1 || ! script --version 2>&1 | grep -qi 'util-linux'; then
  printf 'The live TUI integration requires the util-linux script PTY launcher.\n' >&2
  exit 1
fi
if [[ "$FLUTTER_BIN" == */* && "$FLUTTER_BIN" != /* ]]; then
  FLUTTER_BIN="$(cd -- "$(dirname -- "$FLUTTER_BIN")" && pwd)/$(basename -- "$FLUTTER_BIN")"
fi

BROWSER_WEB_BUILD="${FORGE_WEB_BUILD_DIR:-}"
if [[ "$FORGE_BROWSER_E2E" == "1" ]]; then
  if ! command -v "$FORGE_BROWSER_PYTHON" >/dev/null 2>&1; then
    printf 'Python executable not found: %s\n' "$FORGE_BROWSER_PYTHON" >&2
    exit 1
  fi
  if ! "$FORGE_BROWSER_PYTHON" -c 'from playwright.sync_api import sync_playwright' >/dev/null 2>&1; then
    printf 'The browser E2E requires Python Playwright; install it with %s -m pip install playwright.\n' "$FORGE_BROWSER_PYTHON" >&2
    exit 1
  fi
  if [[ -n "$BROWSER_WEB_BUILD" ]]; then
    if [[ ! -f "$BROWSER_WEB_BUILD/index.html" ]]; then
      printf 'FORGE_WEB_BUILD_DIR must contain a Flutter Web build: %s\n' "$BROWSER_WEB_BUILD" >&2
      exit 1
    fi
  else
    BROWSER_WEB_BUILD="$(mktemp -d)"
    trap 'rm -rf -- "$BROWSER_WEB_BUILD"' EXIT
    (
      cd "$SNAPLINK_CONSOLE_ROOT"
      "$FLUTTER_BIN" build web --no-pub --release --base-href /forge/ --output "$BROWSER_WEB_BUILD"
    )
  fi
fi
if [[ "$FORGE_RUNTIME_BIN" == */* && "$FORGE_RUNTIME_BIN" != /* ]]; then
  FORGE_RUNTIME_BIN="$(cd -- "$(dirname -- "$FORGE_RUNTIME_BIN")" && pwd)/$(basename -- "$FORGE_RUNTIME_BIN")"
fi

(
  cd "$SNAPLINK_CONSOLE_ROOT"
  if [[ ! -f .dart_tool/package_config.json ]]; then
    printf 'Flutter dependencies are not resolved; run flutter pub get in %s first.\n' "$SNAPLINK_CONSOLE_ROOT" >&2
    exit 1
  fi
)

(
  cd "$REPO_ROOT/forge-runtime"
  cargo build -p forge-runtime-cli --bin forge-runtime
)

(
  cd "$REPO_ROOT/forge-core"
  # §698/§699/§703 are exercised inside the shared native lifecycle: the
  # Android-host/iOS-host cold starts consume authenticated SSE, lossless
  # inventory-v2 observation, and one planning-only scheduler preview while
  # the request allowlist has no lease/reservation/dispatch/execution effect.
  FORGE_RUNTIME_BIN="$FORGE_RUNTIME_BIN" \
    FORGE_CONSOLE_E2E=1 \
    FORGE_BROWSER_E2E="$FORGE_BROWSER_E2E" \
    FORGE_CLIENT_INSTANCE_SESSION_PROJECTION_E2E="$FORGE_CLIENT_INSTANCE_SESSION_PROJECTION_E2E" \
    FORGE_CLIENT_INSTANCE_RESOURCE_PROJECTION_E2E="$FORGE_CLIENT_INSTANCE_RESOURCE_PROJECTION_E2E" \
    FORGE_CLIENT_INSTANCE_DISPATCH_PLAN_PROJECTION_E2E="$FORGE_CLIENT_INSTANCE_DISPATCH_PLAN_PROJECTION_E2E" \
    FORGE_CLIENT_INSTANCE_EXECUTION_READINESS_PROJECTION_E2E="$FORGE_CLIENT_INSTANCE_EXECUTION_READINESS_PROJECTION_E2E" \
    FORGE_CLIENT_INSTANCE_EXECUTION_RECONCILIATION_PROJECTION_E2E="$FORGE_CLIENT_INSTANCE_EXECUTION_RECONCILIATION_PROJECTION_E2E" \
    FORGE_WEB_BUILD_DIR="$BROWSER_WEB_BUILD" \
    FORGE_BROWSER_PYTHON="$FORGE_BROWSER_PYTHON" \
    FLUTTER_BIN="$FLUTTER_BIN" \
    SNAPLINK_CONSOLE_ROOT="$SNAPLINK_CONSOLE_ROOT" \
    go test ./internal/appserver -run '^(TestIndependentClientsShareOwnedConversationAndPrompts|TestRunAcceptedInventoryActivationMountsOwnerScopedDeviceRoute|TestSnaplinkAuthenticatedClientInstanceSessionProjectionE2EWhenConfigured|TestSnaplinkAuthenticatedClientInstanceResourceProjectionE2EWhenConfigured|TestSnaplinkAuthenticatedClientInstanceDispatchPlanProjectionE2EWhenConfigured|TestSnaplinkAuthenticatedClientInstanceExecutionReadinessProjectionE2EWhenConfigured|TestSnaplinkAuthenticatedClientInstanceExecutionReconciliationProjectionE2EWhenConfigured)$' -count=1
)
