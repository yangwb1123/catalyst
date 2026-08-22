//go:build unix && !aix && !solaris

package authenticatedadrlifecycleauthority

import (
	"bytes"
	"crypto/ed25519"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"sync"
	"testing"

	approvalauthority "forgeos/forge-core/internal/authenticatedadrapprovalauthority"
)

func TestTransitionStoreAndHistoricalReplay(t *testing.T) {
	fixture := newAuthorityFixture(t)
	proposalBefore := cloneBytes(fixture.proposal)
	authorization := fixture.approvalStored(t)
	input := fixture.lifecycleInput(t, authorization)
	trust := fixture.lifecycleTrust()
	stored, err := TransitionAndStore(fixture.lifecycleConfig, input, authorization, trust)
	if err != nil {
		t.Fatalf("%v: %v", err, errors.Unwrap(err))
	}
	if stored.Sequence() != 1 || stored.Disposition() != "stored" || len(stored.StateJSON()) == 0 {
		t.Fatalf("unexpected stored transition")
	}
	assertLifecycleResult(t, stored, "stored")
	statePath := filepath.Join(fixture.lifecycleConfig.AuthorityRoot, fixture.lifecycleConfig.StateDir, stateFile)
	before := readTestFile(t, statePath)
	replay, err := ReplayStored(fixture.lifecycleConfig, input, trust)
	if err != nil {
		t.Fatal(err)
	}
	after := readTestFile(t, statePath)
	if replay.Sequence() != 1 || replay.Disposition() != "exact_replay" || !bytes.Equal(before, after) {
		t.Fatal("historical replay changed state")
	}
	assertLifecycleResult(t, replay, "exact_replay")
	lateTrust := trust
	lateTrust.ObservedAtUnixMS = testObserved + 400_000
	lateReplay, err := ReplayStored(fixture.lifecycleConfig, input, lateTrust)
	if err != nil || lateReplay.Disposition() != "exact_replay" {
		t.Fatalf("historical replay did not survive upstream windows: %v", err)
	}
	if !bytes.Equal(fixture.proposal, proposalBefore) {
		t.Fatal("immutable proposal bytes changed")
	}
}

func TestHistoricalReplayNeverCreatesMissingLock(t *testing.T) {
	t.Run("empty state", func(t *testing.T) {
		fixture := newAuthorityFixture(t)
		authorization := fixture.approvalStored(t)
		stateDir := lifecycleStateDir(fixture)
		removeLifecycleLock(t, stateDir)
		before := directorySnapshot(t, stateDir)
		_, err := ReplayStored(fixture.lifecycleConfig,
			fixture.lifecycleInput(t, authorization), fixture.lifecycleTrust())
		assertLifecycleCode(t, err, codeStateRejected)
		assertDirectoryUnchanged(t, stateDir, before)
	})
	t.Run("state without lock", func(t *testing.T) {
		fixture := newAuthorityFixture(t)
		authorization := fixture.approvalStored(t)
		input := fixture.lifecycleInput(t, authorization)
		if _, err := TransitionAndStore(fixture.lifecycleConfig, input,
			authorization, fixture.lifecycleTrust()); err != nil {
			t.Fatal(err)
		}
		stateDir := lifecycleStateDir(fixture)
		if err := os.Remove(filepath.Join(stateDir, lockFile)); err != nil {
			t.Fatal(err)
		}
		before := directorySnapshot(t, stateDir)
		_, err := ReplayStored(fixture.lifecycleConfig, input, fixture.lifecycleTrust())
		assertLifecycleCode(t, err, codeStateRejected)
		assertDirectoryUnchanged(t, stateDir, before)
	})
}

func TestTransitionNeverCreatesMissingLock(t *testing.T) {
	t.Run("unauthorized empty state", func(t *testing.T) {
		fixture := newAuthorityFixture(t)
		authorization := fixture.approvalStored(t)
		input := fixture.lifecycleInput(t, authorization)
		stateDir := lifecycleStateDir(fixture)
		removeLifecycleLock(t, stateDir)
		before := directorySnapshot(t, stateDir)
		_, err := TransitionAndStore(fixture.lifecycleConfig, input, nil, fixture.lifecycleTrust())
		assertLifecycleCode(t, err, codeStateRejected)
		assertDirectoryUnchanged(t, stateDir, before)
	})
	t.Run("exact replay", func(t *testing.T) {
		fixture := newAuthorityFixture(t)
		authorization := fixture.approvalStored(t)
		input := fixture.lifecycleInput(t, authorization)
		if _, err := TransitionAndStore(fixture.lifecycleConfig, input,
			authorization, fixture.lifecycleTrust()); err != nil {
			t.Fatal(err)
		}
		stateDir := lifecycleStateDir(fixture)
		removeLifecycleLock(t, stateDir)
		before := directorySnapshot(t, stateDir)
		_, err := TransitionAndStore(fixture.lifecycleConfig, input, nil, fixture.lifecycleTrust())
		assertLifecycleCode(t, err, codeStateRejected)
		assertDirectoryUnchanged(t, stateDir, before)
	})
}

type directoryView struct {
	entries  []string
	mode     os.FileMode
	modified int64
}

func lifecycleStateDir(fixture *authorityFixture) string {
	return filepath.Join(fixture.lifecycleConfig.AuthorityRoot, fixture.lifecycleConfig.StateDir)
}

func removeLifecycleLock(t *testing.T, stateDir string) {
	t.Helper()
	if err := os.Remove(filepath.Join(stateDir, lockFile)); err != nil {
		t.Fatal(err)
	}
}

func directorySnapshot(t *testing.T, path string) directoryView {
	t.Helper()
	entries, err := os.ReadDir(path)
	if err != nil {
		t.Fatal(err)
	}
	names := make([]string, len(entries))
	for index, entry := range entries {
		names[index] = entry.Name()
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	return directoryView{entries: names, mode: info.Mode(), modified: info.ModTime().UnixNano()}
}

func assertDirectoryUnchanged(t *testing.T, path string, before directoryView) {
	t.Helper()
	after := directorySnapshot(t, path)
	if !equalDirectoryEntries(before.entries, after.entries) || before.mode != after.mode ||
		before.modified != after.modified {
		t.Fatalf("replay changed state directory: before=%+v after=%+v", before, after)
	}
}

func equalDirectoryEntries(left, right []string) bool {
	if len(left) != len(right) {
		return false
	}
	for index := range left {
		if left[index] != right[index] {
			return false
		}
	}
	return true
}

func TestStoredTransitionIsOpaqueAndDetached(t *testing.T) {
	zero := new(StoredTransition)
	if zero.ResultJSON() != nil || zero.StateJSON() != nil || zero.Sequence() != 0 || zero.Disposition() != "" {
		t.Fatal("zero StoredTransition became valid")
	}
	fixture := newAuthorityFixture(t)
	authorization := fixture.approvalStored(t)
	stored, err := TransitionAndStore(fixture.lifecycleConfig,
		fixture.lifecycleInput(t, authorization), authorization, fixture.lifecycleTrust())
	if err != nil {
		t.Fatal(err)
	}
	state := stored.StateJSON()
	result := stored.ResultJSON()
	state[0] ^= 1
	result[0] ^= 1
	if bytes.Equal(state, stored.StateJSON()) || bytes.Equal(result, stored.ResultJSON()) {
		t.Fatal("StoredTransition returned aliased bytes")
	}
}

func TestTransitionExactReplayPrecedesCapabilityAndCapacity(t *testing.T) {
	fixture := newAuthorityFixture(t)
	authorization := fixture.approvalStored(t)
	input := fixture.lifecycleInput(t, authorization)
	trust := fixture.lifecycleTrust()
	if _, err := TransitionAndStore(fixture.lifecycleConfig, input, authorization, trust); err != nil {
		t.Fatal(err)
	}
	zero := new(approvalauthority.StoredAuthorization)
	replay, err := TransitionAndStore(fixture.lifecycleConfig, input, zero, trust)
	if err != nil || replay.Disposition() != "exact_replay" {
		t.Fatalf("exact replay required no current capability: %v", err)
	}
}

func TestTransitionRejectsForgedProjectionSignatureAndTrust(t *testing.T) {
	t.Run("zero capability", func(t *testing.T) {
		fixture := newAuthorityFixture(t)
		authorization := fixture.approvalStored(t)
		input := fixture.lifecycleInput(t, authorization)
		_, err := TransitionAndStore(fixture.lifecycleConfig, input, new(approvalauthority.StoredAuthorization), fixture.lifecycleTrust())
		assertLifecycleCode(t, err, codeAuthorizationRejected)
	})
	t.Run("request signature", func(t *testing.T) {
		fixture := newAuthorityFixture(t)
		authorization := fixture.approvalStored(t)
		input := fixture.lifecycleInput(t, authorization)
		request := loadRawObject(t, input.RequestJSON)
		signature := request["signature"].(map[string]any)["signature_base64url"].(string)
		request["signature"].(map[string]any)["signature_base64url"] = mutateFirst(signature)
		input.RequestJSON = canonicalForTest(t, request)
		_, err := TransitionAndStore(fixture.lifecycleConfig, input, authorization, fixture.lifecycleTrust())
		assertLifecycleCode(t, err, codeSignatureRejected)
	})
	t.Run("observation", func(t *testing.T) {
		fixture := newAuthorityFixture(t)
		authorization := fixture.approvalStored(t)
		input := fixture.lifecycleInput(t, authorization)
		trust := fixture.lifecycleTrust()
		trust.ObservedAtUnixMS++
		_, err := TransitionAndStore(fixture.lifecycleConfig, input, authorization, trust)
		assertLifecycleCode(t, err, codeAuthorizationRejected)
	})
}

func TestTransitionIdempotencyConflictAndStateTampering(t *testing.T) {
	fixture := newAuthorityFixture(t)
	authorization := fixture.approvalStored(t)
	input := fixture.lifecycleInput(t, authorization)
	trust := fixture.lifecycleTrust()
	if _, err := TransitionAndStore(fixture.lifecycleConfig, input, authorization, trust); err != nil {
		t.Fatal(err)
	}
	conflict := loadRawObject(t, input.RequestJSON)
	conflict["expires_at_unix_ms"] = conflict["expires_at_unix_ms"].(int64) - 1
	resealLifecycleRequest(t, conflict, fixture.lifecycleRequestPrivate)
	conflicting := EncodedTransitionInput{RequestJSON: canonicalForTest(t, conflict)}
	_, err := ReplayStored(fixture.lifecycleConfig, conflicting, trust)
	assertLifecycleCode(t, err, codeIdempotencyConflict)

	statePath := filepath.Join(fixture.lifecycleConfig.AuthorityRoot, fixture.lifecycleConfig.StateDir, stateFile)
	state := loadRawObject(t, readTestFile(t, statePath))
	state["state_sha256"] = string(bytes.Repeat([]byte{'0'}, 64))
	if err = os.WriteFile(statePath, canonicalForTest(t, state), privateMode); err != nil {
		t.Fatal(err)
	}
	_, err = ReplayStored(fixture.lifecycleConfig, input, trust)
	assertLifecycleCode(t, err, codeStateRejected)
}

func TestFreshTransitionPreservesPublicCASClassification(t *testing.T) {
	mutations := map[string]func(map[string]any){
		"sequence": func(request map[string]any) { request["expected_next_sequence"] = int64(1) },
		"ledger":   func(request map[string]any) { request["expected_ledger_sha256"] = nil },
		"head": func(request map[string]any) {
			request["expected_current_head_set_sha256"] = string(bytes.Repeat([]byte{'0'}, 64))
		},
	}
	for name, mutate := range mutations {
		t.Run(name, func(t *testing.T) {
			fixture, authorization, input := positionedFreshTransition(t)
			request := loadRawObject(t, input.RequestJSON)
			mutate(request)
			resealLifecycleRequest(t, request, fixture.lifecycleRequestPrivate)
			_, err := TransitionAndStore(fixture.lifecycleConfig,
				EncodedTransitionInput{RequestJSON: canonicalForTest(t, request)},
				authorization, fixture.lifecycleTrust())
			assertLifecycleCode(t, err, codeCASConflict)
		})
	}
}

func positionedFreshTransition(t *testing.T) (*authorityFixture,
	*approvalauthority.StoredAuthorization, EncodedTransitionInput) {
	t.Helper()
	fixture := newAuthorityFixture(t)
	fixture.retargetProposal(t, "ADR-9201", "ADR-9201-position-a.md", nil)
	firstAuthorization := fixture.approvalStoredIn(t, "approval-position-a")
	emptyHead := domainDigest(headDomain, []byte("[]"))
	firstInput := fixture.lifecycleInputAt(t, firstAuthorization, 1, nil, emptyHead, []any{})
	first, err := TransitionAndStore(fixture.lifecycleConfig, firstInput,
		firstAuthorization, fixture.lifecycleTrust())
	if err != nil {
		t.Fatal(err)
	}
	fixture.retargetProposal(t, "ADR-9202", "ADR-9202-position-b.md", nil)
	secondAuthorization := fixture.approvalStoredIn(t, "approval-position-b")
	ledger, head, _ := statePosition(t, first.StateJSON())
	return fixture, secondAuthorization,
		fixture.lifecycleInputAt(t, secondAuthorization, 2, ledger, head, []any{})
}

func TestBuildClassificationPreservesCapacityAndCASCodes(t *testing.T) {
	for _, code := range []Code{codeCapacityExhausted, codeCASConflict} {
		classified := classifyBuildError(coded(code, errors.New("classified")))
		assertLifecycleCode(t, classified, code)
	}
	assertLifecycleCode(t, classifyBuildError(errors.New("plain")), codeInputRejected)
}

func TestReplayRejectsTrustedTimeRegression(t *testing.T) {
	fixture := newAuthorityFixture(t)
	authorization := fixture.approvalStored(t)
	input := fixture.lifecycleInput(t, authorization)
	trust := fixture.lifecycleTrust()
	if _, err := TransitionAndStore(fixture.lifecycleConfig, input, authorization, trust); err != nil {
		t.Fatal(err)
	}
	trust.ObservedAtUnixMS--
	_, err := ReplayStored(fixture.lifecycleConfig, input, trust)
	assertLifecycleCode(t, err, codeTimeRejected)
}

func TestConcurrentLifecycleTransitionAppendsAtMostOnce(t *testing.T) {
	fixture := newAuthorityFixture(t)
	authorization := fixture.approvalStored(t)
	input := fixture.lifecycleInput(t, authorization)
	trust := fixture.lifecycleTrust()
	start := make(chan struct{})
	found := make([]error, 2)
	var wait sync.WaitGroup
	for index := range found {
		wait.Add(1)
		go func(slot int) {
			defer wait.Done()
			<-start
			_, found[slot] = TransitionAndStore(fixture.lifecycleConfig, input, authorization, trust)
		}(index)
	}
	close(start)
	wait.Wait()
	successes := 0
	for _, err := range found {
		if err == nil {
			successes++
			continue
		}
		if ErrorCode(err) != codeStateBusy {
			t.Fatalf("unexpected race outcome: %v", ErrorCode(err))
		}
	}
	if successes < 1 {
		t.Fatal("no concurrent transition succeeded")
	}
	state := loadRawObject(t, readTestFile(t, filepath.Join(fixture.lifecycleConfig.AuthorityRoot, fixture.lifecycleConfig.StateDir, stateFile)))
	ledger := state["ledger"].(map[string]any)
	if len(ledger["entries"].([]any)) != 1 {
		t.Fatal("race appended more than once")
	}
}

func TestLifecycleAtomicTwoTargetSupersessionAndCAS(t *testing.T) {
	fixture := newAuthorityFixture(t)
	fixture.retargetProposal(t, "ADR-9101", "ADR-9101-lifecycle-head-a.md", nil)
	firstAuthorization := fixture.approvalStoredIn(t, "approval-state-a")
	emptyHead := domainDigest(headDomain, []byte("[]"))
	firstInput := fixture.lifecycleInputAt(t, firstAuthorization, 1, nil, emptyHead, []any{})
	first, err := TransitionAndStore(fixture.lifecycleConfig, firstInput,
		firstAuthorization, fixture.lifecycleTrust())
	if err != nil {
		t.Fatal(err)
	}

	fixture.retargetProposal(t, "ADR-9102", "ADR-9102-lifecycle-head-b.md", nil)
	secondAuthorization := fixture.approvalStoredIn(t, "approval-state-b")
	ledger, head, _ := statePosition(t, first.StateJSON())
	secondInput := fixture.lifecycleInputAt(t, secondAuthorization, 2, ledger, head, []any{})
	second, err := TransitionAndStore(fixture.lifecycleConfig, secondInput,
		secondAuthorization, fixture.lifecycleTrust())
	if err != nil {
		t.Fatal(err)
	}

	fixture.retargetProposal(t, "ADR-9103", "ADR-9103-lifecycle-join.md",
		[]string{"ADR-9101", "ADR-9102"})
	thirdAuthorization := fixture.approvalStoredIn(t, "approval-state-c")
	ledger, head, decisions := statePosition(t, second.StateJSON())
	targets := targetRefs(decisions, "ADR-9101", "ADR-9102")
	thirdInput := fixture.lifecycleInputAt(t, thirdAuthorization, 3, ledger, head, targets)
	third, err := TransitionAndStore(fixture.lifecycleConfig, thirdInput,
		thirdAuthorization, fixture.lifecycleTrust())
	if err != nil {
		t.Fatal(err)
	}
	assertAtomicJoinAndConflict(t, fixture, third, thirdInput, emptyHead)
}

func assertAtomicJoinAndConflict(t *testing.T, fixture *authorityFixture,
	third *StoredTransition, thirdInput EncodedTransitionInput, emptyHead string) {
	t.Helper()
	_, _, decisions := statePosition(t, third.StateJSON())
	statuses := map[string]string{}
	for _, raw := range decisions {
		node := raw.(map[string]any)
		statuses[node["adr_id"].(string)] = node["status"].(string)
	}
	if statuses["ADR-9101"] != "superseded" || statuses["ADR-9102"] != "superseded" ||
		statuses["ADR-9103"] != "accepted" {
		t.Fatalf("atomic join differs: %v", statuses)
	}

	stale := loadRawObject(t, thirdInput.RequestJSON)
	stale["expected_current_head_set_sha256"] = emptyHead
	resealLifecycleRequest(t, stale, fixture.lifecycleRequestPrivate)
	_, err := ReplayStored(fixture.lifecycleConfig,
		EncodedTransitionInput{RequestJSON: canonicalForTest(t, stale)}, fixture.lifecycleTrust())
	assertLifecycleCode(t, err, codeIdempotencyConflict)
}

func assertLifecycleResult(t *testing.T, stored *StoredTransition, disposition string) {
	t.Helper()
	var result map[string]any
	decoder := json.NewDecoder(bytes.NewReader(stored.ResultJSON()))
	decoder.UseNumber()
	if err := decoder.Decode(&result); err != nil || result["delivery_disposition"] != disposition {
		t.Fatalf("result differs: %v", err)
	}
	state := stored.StateJSON()
	fixture := new(map[string]any)
	_ = fixture
	if len(state) == 0 {
		t.Fatal("state is absent")
	}
}

func loadRawObject(t *testing.T, raw []byte) map[string]any {
	t.Helper()
	value, err := parseCanonicalJSON(raw, maxBundle, "test object")
	if err != nil {
		t.Fatal(err)
	}
	return value.(map[string]any)
}

func resealLifecycleRequest(t *testing.T, request map[string]any,
	private ed25519.PrivateKey) {
	t.Helper()
	request["request_id"], request["request_sha256"] = "", ""
	request["signature"].(map[string]any)["signature_base64url"] = ""
	digest, err := digestFor("request", request)
	if err != nil {
		t.Fatal(err)
	}
	request["request_sha256"] = digest
	request["request_id"] = "architecture-decision-lifecycle-request-" + digest
	request["signature"].(map[string]any)["signature_base64url"] = signTestDigest(t, private, requestSignDomain, digest)
}

func mutateFirst(value string) string {
	replacement := byte('A')
	if value[0] == replacement {
		replacement = 'B'
	}
	return string(replacement) + value[1:]
}

func assertLifecycleCode(t *testing.T, err error, want Code) {
	t.Helper()
	if got := ErrorCode(err); got != want {
		t.Fatalf("code=%q want=%q err=%v", got, want, err)
	}
}
