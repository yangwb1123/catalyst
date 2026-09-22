package auditprojection

import (
	"bytes"
	"encoding/json"
	"io"
	"strings"
	"testing"
	"time"
)

// TestPromptAcceptedProjectionFitsAuditGovernanceAcceptedTopic checks the
// read-only seam against the downstream accepted-event envelope. The test
// models the required wire invariants from Audit Governance's
// internal/kafka/schema.go without importing or invoking that service. Forge
// remains responsible only for projection; this test does not publish,
// persist, or authorize an event.
func TestPromptAcceptedProjectionFitsAuditGovernanceAcceptedTopic(t *testing.T) {
	event, err := Project(projectionOwner(), projectionChange())
	if err != nil {
		t.Fatal(err)
	}
	wire, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}
	fields := decodeSingleJSONObject(t, wire)
	assertAcceptedEnvelopeFields(t, fields, event.EventID)
	assertAcceptedActorAndPayload(t, fields)
}

func assertAcceptedEnvelopeFields(t *testing.T, fields map[string]json.RawMessage, eventID string) {
	t.Helper()
	for _, name := range []string{
		"event_id", "source_system", "event_type", "schema_id", "schema_version",
		"occurred_at", "actor", "action", "outcome", "data_classification",
		"retention_class", "idempotency_key", "payload",
	} {
		if _, ok := fields[name]; !ok {
			t.Fatalf("Audit Governance accepted envelope omitted required field %q", name)
		}
	}
	for _, name := range []string{
		"event_id", "source_system", "event_type", "schema_id", "action", "outcome",
		"data_classification", "retention_class", "idempotency_key",
	} {
		if value := jsonString(fields[name]); strings.TrimSpace(value) == "" {
			t.Fatalf("Audit Governance accepted envelope field %q is empty", name)
		}
	}
	if got := jsonString(fields["event_id"]); got != eventID {
		t.Fatalf("event_id = %q, want %q", got, eventID)
	}
	if got := jsonString(fields["idempotency_key"]); got != eventID {
		t.Fatalf("idempotency_key = %q, want event_id %q", got, eventID)
	}
	acceptedTopicKey := []byte(eventID)
	if !bytes.Equal(acceptedTopicKey, []byte(jsonString(fields["event_id"]))) {
		t.Fatal("accepted-topic Kafka key would not equal event_id")
	}

	var schemaVersion int
	if err := json.Unmarshal(fields["schema_version"], &schemaVersion); err != nil || schemaVersion < 1 {
		t.Fatalf("schema_version is not a positive integer: %s", fields["schema_version"])
	}
	if _, err := time.Parse(time.RFC3339Nano, jsonString(fields["occurred_at"])); err != nil {
		t.Fatalf("occurred_at is not an RFC3339 timestamp: %v", err)
	}
}

func assertAcceptedActorAndPayload(t *testing.T, fields map[string]json.RawMessage) {
	t.Helper()
	var actor map[string]json.RawMessage
	if err := json.Unmarshal(fields["actor"], &actor); err != nil || actor == nil {
		t.Fatalf("actor is not an object: %s", fields["actor"])
	}
	if strings.TrimSpace(jsonString(actor["id"])) == "" || jsonString(actor["type"]) != "user" {
		t.Fatalf("actor does not preserve the pseudonymized user contract: %s", fields["actor"])
	}

	var payload map[string]json.RawMessage
	if err := json.Unmarshal(fields["payload"], &payload); err != nil || payload == nil {
		t.Fatalf("payload is not an object: %s", fields["payload"])
	}
	contentIncluded, present := payload["content_included"]
	if len(payload) != 1 || !present || jsonBool(contentIncluded) {
		t.Fatalf("payload must contain only content_included=false: %s", fields["payload"])
	}
	for _, forbidden := range []string{"content", "prompt", "title", "provider", "token"} {
		if _, ok := payload[forbidden]; ok {
			t.Fatalf("payload leaked forbidden field %q", forbidden)
		}
	}
}

func decodeSingleJSONObject(t *testing.T, wire []byte) map[string]json.RawMessage {
	t.Helper()
	decoder := json.NewDecoder(bytes.NewReader(wire))
	var fields map[string]json.RawMessage
	if err := decoder.Decode(&fields); err != nil || fields == nil {
		t.Fatalf("event is not one JSON object: %v", err)
	}
	var trailing json.RawMessage
	if err := decoder.Decode(&trailing); err != io.EOF {
		t.Fatalf("event contains more than one JSON value: %s", trailing)
	}
	return fields
}

func jsonString(raw json.RawMessage) string {
	var value string
	if err := json.Unmarshal(raw, &value); err != nil {
		return ""
	}
	return value
}

func jsonBool(raw json.RawMessage) bool {
	var value bool
	return json.Unmarshal(raw, &value) == nil && value
}
