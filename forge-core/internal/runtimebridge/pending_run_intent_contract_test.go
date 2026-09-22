package runtimebridge

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"testing"

	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

type pendingRunIntentContractFixture struct {
	SchemaVersion  string                                        `json:"schema_version"`
	EvaluationMode string                                        `json:"evaluation_mode"`
	Authority      pendingRunIntentContractAuthority             `json:"authority"`
	Owner          model.Owner                                   `json:"owner"`
	ConversationID string                                        `json:"conversation_id"`
	Submission     intentmodel.PendingRunIntentSubmissionResult  `json:"submission"`
	Page           intentmodel.OwnedPendingRunIntentPage         `json:"page"`
	Timeline       intentmodel.OwnedPendingRunIntentTimelinePage `json:"timeline"`
	Expected       pendingRunIntentContractExpected              `json:"expected"`
}

type pendingRunIntentContractAuthority struct {
	DeviceIdentityVerified bool `json:"device_identity_verified"`
	InventoryAuthoritative bool `json:"inventory_authoritative"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
	RunCreated             bool `json:"run_created"`
	AuditPublished         bool `json:"audit_published"`
}

type pendingRunIntentContractExpected struct {
	PromptRole         string `json:"prompt_role"`
	IntentStatus       string `json:"intent_status"`
	InitialEventType   string `json:"initial_event_type"`
	TimelineEventCount int    `json:"timeline_event_count"`
	Replayed           bool   `json:"replayed"`
}

func TestPendingRunIntentContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_PENDING_RUN_INTENT_FIXTURE")
	if path == "" {
		t.Skip("FORGE_PENDING_RUN_INTENT_FIXTURE is set by the cross-repository contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var fixture pendingRunIntentContractFixture
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode pending Run-intent fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("fixture has trailing JSON: %v", err)
	}
	if fixture.SchemaVersion != "forge.pending-run-intent/v1" ||
		fixture.EvaluationMode != "owner_scoped_pending_intent_preview" ||
		fixture.ConversationID == "" || fixture.Owner.Issuer == "" ||
		fixture.Owner.Subject == "" || fixture.Owner.TenantID == "" {
		t.Fatalf("invalid pending Run-intent envelope: %#v", fixture)
	}
	if fixture.Authority.DeviceIdentityVerified || fixture.Authority.InventoryAuthoritative ||
		fixture.Authority.ReservationCreated || fixture.Authority.ExecutionAuthorized ||
		fixture.Authority.DispatchPerformed || fixture.Authority.RunCreated ||
		fixture.Authority.AuditPublished {
		t.Fatal("pending Run-intent fixture claims authority")
	}
	if fixture.Submission.Prompt.ConversationID != fixture.ConversationID ||
		fixture.Submission.Prompt.Role != "user" || fixture.Submission.Prompt.Content == "" ||
		fixture.Submission.Prompt.ID != fixture.Submission.Intent.PromptID ||
		fixture.Submission.Prompt.CreatedAtMS != fixture.Submission.Intent.SubmittedAtMS ||
		fixture.Submission.Intent.ConversationID != fixture.ConversationID ||
		fixture.Submission.Intent.LatestSequence != 1 || fixture.Submission.Intent.Status != "pending" ||
		fixture.Submission.InitialEvent.Sequence != 1 || fixture.Submission.InitialEvent.Type != "submitted" ||
		fixture.Submission.InitialEvent.EmittedAtMS != fixture.Submission.Intent.SubmittedAtMS {
		t.Fatalf("invalid pending Run-intent submission: %#v", fixture.Submission)
	}
	if fixture.Expected.PromptRole != fixture.Submission.Prompt.Role ||
		fixture.Expected.IntentStatus != fixture.Submission.Intent.Status ||
		fixture.Expected.InitialEventType != fixture.Submission.InitialEvent.Type ||
		fixture.Expected.TimelineEventCount != 1 || fixture.Expected.Replayed != fixture.Submission.Replayed {
		t.Fatalf("pending Run-intent expectation drift: %#v", fixture.Expected)
	}
	if fixture.Page.ConversationID != fixture.ConversationID || fixture.Page.HasMore ||
		fixture.Page.NextCursor != nil || len(fixture.Page.Intents) != 1 {
		t.Fatalf("invalid pending Run-intent page: %#v", fixture.Page)
	}
	intent := fixture.Page.Intents[0]
	if intent != fixture.Submission.Intent {
		t.Fatalf("page intent=%#v differs from submission intent=%#v", intent, fixture.Submission.Intent)
	}
	if fixture.Timeline.ConversationID != fixture.ConversationID ||
		fixture.Timeline.IntentID != intent.IntentID || fixture.Timeline.AfterSequence != 0 ||
		fixture.Timeline.ScannedThroughSequence != 1 || fixture.Timeline.HasMore ||
		len(fixture.Timeline.Events) != fixture.Expected.TimelineEventCount ||
		fixture.Timeline.Events[0] != fixture.Submission.InitialEvent {
		t.Fatalf("invalid pending Run-intent timeline: %#v", fixture.Timeline)
	}
}
