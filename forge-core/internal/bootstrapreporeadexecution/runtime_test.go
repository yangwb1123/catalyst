//go:build linux && (amd64 || arm64)

package bootstrapreporeadexecution

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/pinnedreporead"
)

type sequenceClock struct {
	calls  int
	values []int64
}

func (clock *sequenceClock) nowUnixMilli() (int64, error) {
	if clock.calls >= len(clock.values) {
		return 0, fmt.Errorf("unexpected clock read")
	}
	value := clock.values[clock.calls]
	clock.calls++
	return value, nil
}

type runtimeLayout struct {
	authority, repo, stateLedger string
	config                       Config
}

func TestRuntimeStoresRawFirstDeliveryThenReplaysWithoutRepository(t *testing.T) {
	bundle := newRuntimeBundle(t)
	layout := installRuntimeBundle(t, bundle)
	clock := &sequenceClock{values: []int64{1_700_000_005_000, 1_700_000_006_000,
		1_700_000_007_000}}
	output, err := executeWith(layout.config, runtimeDependencies(clock, 17*time.Millisecond))
	if err != nil {
		t.Fatal(err)
	}
	first := decodeTestJSON(t, output)
	if first["delivery_disposition"] != "first_delivery" || first["execution_result"] == nil {
		t.Fatalf("first delivery omitted raw result: %s", output)
	}
	ledgerBefore, err := os.ReadFile(layout.stateLedger)
	if err != nil {
		t.Fatal(err)
	}
	assertUsageTimeline(t, ledgerBefore, 1_700_000_005_000,
		1_700_000_006_000, 1_700_000_007_000)
	removeReplayInputs(t, layout)
	replayCalls := 0
	replayDeps := replayOnlyDependencies(&replayCalls)
	replayed, err := executeWith(layout.config, replayDeps)
	if err != nil {
		t.Fatal(err)
	}
	second := decodeTestJSON(t, replayed)
	assertContentFreeReplay(t, first, second)
	replaceReplayIdentityWithDigests(t, layout, bundle)
	digestCalls := 0
	digestReplay, err := executeWith(layout.config, replayOnlyDependencies(&digestCalls))
	if err != nil {
		t.Fatal(err)
	}
	assertContentFreeReplay(t, first, decodeTestJSON(t, digestReplay))
	ledgerAfter, _ := os.ReadFile(layout.stateLedger)
	if replayCalls != 0 || digestCalls != 0 || !bytes.Equal(ledgerBefore, ledgerAfter) {
		t.Fatal("exact replay touched clock, reader, or durable usage state")
	}
}

func TestRuntimeContentMismatchConsumesGrantWithoutRawReplay(t *testing.T) {
	bundle := newRuntimeBundle(t)
	layout := installRuntimeBundle(t, bundle)
	contents := fixtureRepositoryContent(t, bundle.fixture)
	for path, content := range contents {
		mutated := append([]byte(nil), content...)
		mutated[0] ^= 0xff
		writeRuntimeFile(t, layout.repo, path, mutated)
		break
	}
	clock := &sequenceClock{values: []int64{1_700_000_005_000, 1_700_000_006_000,
		1_700_000_007_000}}
	_, err := executeWith(layout.config, runtimeDependencies(clock, time.Millisecond))
	if ErrorCode(err) != CodeRepositoryRejected {
		t.Fatalf("content mismatch = %q, %v", ErrorCode(err), err)
	}
	removeReplayInputs(t, layout)
	replayed, err := executeWith(layout.config, replayOnlyDependencies(new(int)))
	if err != nil {
		t.Fatal(err)
	}
	delivery := decodeTestJSON(t, replayed)
	receipt := delivery["receipt"].(map[string]any)
	if delivery["execution_result"] != nil || delivery["result_metadata"] != nil ||
		receipt["state"] != "failed_consumed" || receipt["reason_code"] != "content_mismatch" {
		t.Fatalf("failed replay leaked or drifted: %s", replayed)
	}
}

func TestRuntimeMayFinishAfterInvocationExpiryWhenElapsedBudgetIsMet(t *testing.T) {
	bundle := newRuntimeBundle(t)
	layout := installRuntimeBundle(t, bundle)
	clock := &sequenceClock{values: []int64{1_700_000_005_000, 1_700_000_006_000,
		1_700_000_304_000}}
	output, err := executeWith(layout.config, runtimeDependencies(clock, 17*time.Millisecond))
	if err != nil {
		t.Fatal(err)
	}
	delivery := decodeTestJSON(t, output)
	receipt := delivery["receipt"].(map[string]any)
	if receipt["state"] != "completed" || delivery["execution_result"] == nil {
		t.Fatalf("post-expiry terminal did not complete: %s", output)
	}
}

func TestDigestOnlyReplayMissAndOneSidedMatchNeverTouchRepository(t *testing.T) {
	bundle := newRuntimeBundle(t)
	layout := installRuntimeBundle(t, bundle)
	clock := &sequenceClock{values: []int64{1_700_000_005_000, 1_700_000_006_000,
		1_700_000_007_000}}
	if _, err := executeWith(layout.config,
		runtimeDependencies(clock, time.Millisecond)); err != nil {
		t.Fatal(err)
	}
	removeReplayInputs(t, layout)
	calls := 0
	writeReplayDigests(t, layout, strings.Repeat("0", 64), strings.Repeat("1", 64))
	if _, err := executeWith(layout.config, replayOnlyDependencies(&calls)); ErrorCode(err) != CodeInvocationRejected || calls != 0 {
		t.Fatalf("digest miss = %q, calls=%d", ErrorCode(err), calls)
	}
	policy := decodeTestJSON(t, bundle.policy)["execution_policy_sha256"].(string)
	writeReplayDigests(t, layout, policy, strings.Repeat("0", 64))
	if _, err := executeWith(layout.config, replayOnlyDependencies(&calls)); ErrorCode(err) != CodeIdempotencyConflict || calls != 0 {
		t.Fatalf("one-sided digest = %q, calls=%d", ErrorCode(err), calls)
	}
}

func runtimeDependencies(runtimeClock clock, elapsed time.Duration) dependencies {
	times := []time.Time{time.Unix(0, 0), time.Unix(0, 0).Add(elapsed)}
	index := 0
	return dependencies{checkPlatform: pinnedreporead.CheckPlatform,
		preflightRepository: pinnedreporead.PreflightRepository, clock: runtimeClock,
		monotonicNow: func() time.Time {
			value := times[index]
			index++
			return value
		}, openState: openUsageState, readFiles: pinnedreporead.Read}
}

func replayOnlyDependencies(calls *int) dependencies {
	return dependencies{checkPlatform: pinnedreporead.CheckPlatform,
		preflightRepository: func(*os.File) error {
			panic("exact replay probed repository")
		},
		clock: &countingRejectedClock{calls: calls}, monotonicNow: func() time.Time {
			panic("exact replay read monotonic clock")
		}, openState: openUsageState,
		readFiles: func(context.Context, *os.File, []pinnedreporead.ExpectedEntry,
			pinnedreporead.Limits) ([]pinnedreporead.File, error) {
			(*calls)++
			return nil, fmt.Errorf("exact replay invoked reader")
		}}
}

type countingRejectedClock struct{ calls *int }

func (clock *countingRejectedClock) nowUnixMilli() (int64, error) {
	(*clock.calls)++
	return 0, fmt.Errorf("exact replay invoked wall clock")
}

func installRuntimeBundle(t *testing.T, bundle runtimeBundle) runtimeLayout {
	t.Helper()
	base := t.TempDir()
	authority, repo := filepath.Join(base, "authority"), filepath.Join(base, "repository")
	if err := os.Mkdir(authority, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(filepath.Join(authority, "state"), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(repo, 0o700); err != nil {
		t.Fatal(err)
	}
	config := runtimeConfig(authority, repo, bundle)
	writeBundleLeaves(t, authority, config, bundle)
	for path, content := range fixtureRepositoryContent(t, bundle.fixture) {
		writeRuntimeFile(t, repo, path, content)
	}
	return runtimeLayout{authority: authority, repo: repo,
		stateLedger: filepath.Join(authority, "state", "usage-ledger.v1.json"), config: config}
}

func runtimeConfig(authority, repo string, bundle runtimeBundle) Config {
	return Config{RepositoryRoot: repo, AuthorityRoot: authority, StateDir: "state",
		IssuanceTrustRootPath:  "inputs/issuance-root.json",
		IssuanceLedgerPath:     "inputs/issuance-ledger.json",
		ExecutionTrustRootPath: "inputs/execution-root.json",
		ExecutionPolicyPath:    "inputs/execution-policy.json",
		InvocationPath:         "inputs/invocation.json", ManifestPath: "inputs/manifest.json",
		ReceiptSeedPath:           "keys/execution-receipt.seed",
		PinnedIssuanceRootSHA256:  bundle.issuancePin,
		PinnedExecutionRootSHA256: bundle.executionPin}
}

func writeBundleLeaves(t *testing.T, authority string, config Config, bundle runtimeBundle) {
	t.Helper()
	values := map[string][]byte{config.IssuanceTrustRootPath: bundle.issuanceRoot,
		config.IssuanceLedgerPath:     bundle.issuanceLedger,
		config.ExecutionTrustRootPath: bundle.executionRoot,
		config.ExecutionPolicyPath:    bundle.policy, config.InvocationPath: bundle.invocation,
		config.ManifestPath: bundle.manifest, config.ReceiptSeedPath: bundle.seed}
	for path, value := range values {
		writeRuntimeFile(t, authority, path, value)
	}
}

func removeReplayInputs(t *testing.T, layout runtimeLayout) {
	t.Helper()
	if err := os.RemoveAll(layout.repo); err != nil {
		t.Fatal(err)
	}
	for _, relative := range []string{layout.config.ManifestPath, layout.config.ReceiptSeedPath} {
		if err := os.Remove(filepath.Join(layout.authority, relative)); err != nil {
			t.Fatal(err)
		}
	}
}

func replaceReplayIdentityWithDigests(t *testing.T, layout runtimeLayout,
	bundle runtimeBundle) {
	t.Helper()
	policy := decodeTestJSON(t, bundle.policy)
	invocation := decodeTestJSON(t, bundle.invocation)
	writeReplayDigests(t, layout, policy["execution_policy_sha256"].(string),
		invocation["invocation_sha256"].(string))
}

func writeReplayDigests(t *testing.T, layout runtimeLayout, policy, invocation string) {
	t.Helper()
	values := map[string]string{layout.config.ExecutionPolicyPath: policy,
		layout.config.InvocationPath: invocation}
	for relative, value := range values {
		if err := os.WriteFile(filepath.Join(layout.authority, relative), []byte(value), 0o600); err != nil {
			t.Fatal(err)
		}
	}
}

func assertContentFreeReplay(t *testing.T, first, second map[string]any) {
	t.Helper()
	if second["delivery_disposition"] != "exact_replay" || second["execution_result"] != nil {
		t.Fatalf("replay returned raw result: %#v", second)
	}
	if !equalTestNode(first["receipt"], second["receipt"]) ||
		!equalTestNode(first["result_metadata"], second["result_metadata"]) {
		t.Fatal("replay receipt or metadata differs from first delivery")
	}
}

func equalTestNode(left, right any) bool {
	return bytes.Equal(mustJSON(left), mustJSON(right))
}

func mustJSON(value any) []byte {
	data, _ := json.Marshal(value)
	return data
}
