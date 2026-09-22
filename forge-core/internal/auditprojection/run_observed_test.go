package auditprojection

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"path/filepath"
	"testing"

	"forgeos/forge-core/internal/runtimebridge/model"
	"forgeos/forge-core/internal/runtimebridge/runmodel"
)

func TestRunObservedProjectionMatchesStrictGoldenFixture(t *testing.T) {
	actual, err := ProjectRunObserved(runObservedOwner(), "conversation-001", runObservedSummary())
	if err != nil {
		t.Fatal(err)
	}
	encoded, err := json.Marshal(actual)
	if err != nil {
		t.Fatal(err)
	}
	assertRunObservedWireIsClosedAndContentFree(t, encoded)
	fixture, err := os.ReadFile(runObservedFixturePath())
	if err != nil {
		t.Fatal(err)
	}
	var expected RunObserved
	decodeRunObservedFixture(t, fixture, &expected)
	var received RunObserved
	decodeRunObservedFixture(t, encoded, &received)
	if received != expected {
		t.Fatalf("Run observed projection = %#v, want %#v", received, expected)
	}
}

func TestRunObservedFixtureIsClosedAndContentFree(t *testing.T) {
	fixture, err := os.ReadFile(runObservedFixturePath())
	if err != nil {
		t.Fatal(err)
	}
	assertRunObservedWireIsClosedAndContentFree(t, fixture)
}

func assertRunObservedWireIsClosedAndContentFree(t *testing.T, encoded []byte) {
	t.Helper()
	var value map[string]json.RawMessage
	decodeRunObservedFixture(t, encoded, &value)
	want := map[string]struct{}{
		"api_version": {}, "owner_ref": {}, "conversation_id": {}, "run_id": {}, "prompt_id": {},
		"created_at_ms": {}, "latest_sequence": {}, "status": {}, "metadata_observed": {},
		"content_included": {}, "authority": {},
	}
	if len(value) != len(want) {
		t.Fatalf("fixture fields = %v, want exactly %v", value, want)
	}
	for field := range value {
		if _, ok := want[field]; !ok {
			t.Fatalf("fixture contains uncontracted field %q", field)
		}
	}
	for _, forbidden := range []string{
		"content", "prompt_content", "message", "output", "tool", "error", "title", "path", "provider", "token",
		"issuer", "subject", "tenant_id", "authorization", "execution", "dispatch", "outbox",
	} {
		if bytes.Contains(bytes.ToLower(encoded), []byte(`"`+forbidden+`"`)) {
			t.Fatalf("Run observed wire leaks forbidden field %q", forbidden)
		}
	}
	var authority map[string]bool
	decodeRunObservedFixture(t, value["authority"], &authority)
	if len(authority) != 8 {
		t.Fatalf("authority fields = %#v", authority)
	}
	for field, granted := range authority {
		if granted {
			t.Fatalf("authority %q = true", field)
		}
	}
}

func TestProjectRunObservedRejectsInvalidMetadata(t *testing.T) {
	tests := []struct {
		name           string
		owner          model.Owner
		conversationID string
		run            runmodel.OwnedRunSummary
	}{
		{name: "empty owner", owner: model.Owner{}, conversationID: "conversation-001", run: runObservedSummary()},
		{name: "invalid conversation", owner: runObservedOwner(), conversationID: "conversation/001", run: runObservedSummary()},
		{name: "empty run", owner: runObservedOwner(), conversationID: "conversation-001", run: runmodel.OwnedRunSummary{}},
		{name: "zero sequence", owner: runObservedOwner(), conversationID: "conversation-001", run: runObservedSummaryWith(func(run *runmodel.OwnedRunSummary) { run.LatestSequence = 0 })},
		{name: "unsafe timestamp", owner: runObservedOwner(), conversationID: "conversation-001", run: runObservedSummaryWith(func(run *runmodel.OwnedRunSummary) { run.CreatedAtMS = maxObservedSafeInteger + 1 })},
		{name: "unknown status", owner: runObservedOwner(), conversationID: "conversation-001", run: runObservedSummaryWith(func(run *runmodel.OwnedRunSummary) { run.Status = "running" })},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			_, err := ProjectRunObserved(test.owner, test.conversationID, test.run)
			if !errors.Is(err, ErrInvalidProjection) {
				t.Fatalf("ProjectRunObserved error = %v", err)
			}
		})
	}
}

func decodeRunObservedFixture(t *testing.T, encoded []byte, destination any) {
	t.Helper()
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(destination); err != nil {
		t.Fatalf("strict fixture decode: %v", err)
	}
	if err := decoder.Decode(new(any)); !errors.Is(err, io.EOF) {
		t.Fatalf("fixture has trailing JSON: %v", err)
	}
}

func runObservedFixturePath() string {
	return filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "forge-run-observed-v1.json")
}

func runObservedOwner() model.Owner {
	return model.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
}

func runObservedSummary() runmodel.OwnedRunSummary {
	return runmodel.OwnedRunSummary{
		RunID: "run-001", PromptID: "prompt-001", CreatedAtMS: 200, LatestSequence: 5, Status: "nonterminal",
	}
}

func runObservedSummaryWith(update func(*runmodel.OwnedRunSummary)) runmodel.OwnedRunSummary {
	run := runObservedSummary()
	update(&run)
	return run
}
