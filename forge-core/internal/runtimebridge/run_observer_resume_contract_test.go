package runtimebridge

import (
	"encoding/json"
	"os"
	"testing"

	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
)

// TestRunObserverResumeContractFixture keeps the read-only Run timeline
// resume cursor identical across the Go bridge, Rust CLI, and Flutter client.
// It describes metadata pages only; it does not create or resume a Run.
func TestRunObserverResumeContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_RUN_RESUME_CONTRACT_FIXTURE")
	if path == "" {
		t.Skip("FORGE_RUN_RESUME_CONTRACT_FIXTURE is set by the cross-repository contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var envelope struct {
		APIVersion        string            `json:"api_version"`
		ConversationID    string            `json:"conversation_id"`
		RunID             string            `json:"run_id"`
		Limit             int               `json:"limit"`
		Pages             []json.RawMessage `json:"pages"`
		ExpectedSequences []uint64          `json:"expected_sequences"`
	}
	if err := decodeStrict(data, &envelope); err != nil {
		t.Fatalf("decode Run observer resume fixture: %v", err)
	}
	if envelope.APIVersion != "forgeos.run-observer-resume-contract/v1" ||
		envelope.ConversationID != "conversation-001" || envelope.RunID != "run-001" ||
		envelope.Limit != 2 || len(envelope.Pages) != 3 {
		t.Fatalf("unexpected Run observer resume envelope: %#v", envelope)
	}
	if len(envelope.ExpectedSequences) == 0 {
		t.Fatal("Run observer resume fixture has no expected sequence")
	}

	after := uint64(0)
	var sequences []uint64
	for index, rawPage := range envelope.Pages {
		var page runmodel.OwnedRunTimelinePage
		if err := decodeStrict(rawPage, &page); err != nil ||
			!validOwnedRunTimelinePage(rawPage, page, envelope.ConversationID, envelope.RunID, after, envelope.Limit) {
			t.Fatalf("invalid Run observer resume page %d: %v %#v", index, err, page)
		}
		if page.AfterSequence != after {
			t.Fatalf("page %d resumed from %d, want %d", index, page.AfterSequence, after)
		}
		for _, event := range page.Events {
			sequences = append(sequences, event.Sequence)
		}
		after = page.ScannedThroughSequence
	}
	if len(sequences) != len(envelope.ExpectedSequences) {
		t.Fatalf("observed sequences=%v, want=%v", sequences, envelope.ExpectedSequences)
	}
	for index, sequence := range sequences {
		if sequence != envelope.ExpectedSequences[index] {
			t.Fatalf("observed sequences=%v, want=%v", sequences, envelope.ExpectedSequences)
		}
	}
}
