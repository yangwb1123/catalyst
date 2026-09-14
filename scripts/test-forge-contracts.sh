#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
SNAPLINK_CONSOLE_ROOT="${SNAPLINK_CONSOLE_ROOT:-$(dirname -- "$REPO_ROOT")/workspace/demo/snaplink-console}"
FORGE_CONTRACT_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-owned-conversation-page-v1.json"
FORGE_PLACEMENT_PARITY_FIXTURE="$REPO_ROOT/docs/contracts/fixtures/forge-device-placement-policy-parity-v1.json"

if [[ ! -f "$FORGE_CONTRACT_FIXTURE" ]]; then
  printf 'Missing Forge conversation contract fixture: %s\n' "$FORGE_CONTRACT_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$FORGE_PLACEMENT_PARITY_FIXTURE" ]]; then
  printf 'Missing Forge placement parity fixture: %s\n' "$FORGE_PLACEMENT_PARITY_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$SNAPLINK_CONSOLE_ROOT/pubspec.yaml" ]]; then
  printf 'Flutter Console repo not found: %s\n' "$SNAPLINK_CONSOLE_ROOT" >&2
  exit 1
fi

export FORGE_CONTRACT_FIXTURE

(
  cd "$REPO_ROOT/forge-core"
  go test ./internal/runtimebridge/model -run '^TestOwnedConversationContractFixture$' -count=1
  go test ./internal/deviceplacement -run '^TestPolicyParityFixture$' -count=1
)

(
  cd "$REPO_ROOT/forge-runtime"
  cargo test -p forge-runtime-cli conversation_contract_fixture
  cargo test -p forge-runtime-domain shared_policy_fixture_matches_offline_go_projection
)

(
  cd "$SNAPLINK_CONSOLE_ROOT"
  flutter test test/forge_conversations_contract_test.dart
)
