//go:build linux && !android

package controlstore

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	core "forgeos/forge-core/internal/platformcorecontract"
)

const testUnixMS = int64(1788048000000)

type testStore struct {
	store *Store
	root  *os.Root
	path  string
}

func openTestStore(t *testing.T) testStore {
	t.Helper()
	parent := t.TempDir()
	if err := os.Chmod(parent, 0o700); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(parent, "state")
	if err := os.Mkdir(path, 0o700); err != nil {
		t.Fatal(err)
	}
	root, err := os.OpenRoot(path)
	if err != nil {
		t.Fatal(err)
	}
	store, err := OpenBound(context.Background(), root)
	if err != nil {
		_ = root.Close()
		t.Fatal(err)
	}
	value := testStore{store: store, root: root, path: path}
	t.Cleanup(func() { value.close() })
	return value
}

func (value testStore) close() {
	_ = value.store.Close()
	_ = value.root.Close()
}

func reopenTestStore(t *testing.T, path string) (*Store, *os.Root) {
	t.Helper()
	root, err := os.OpenRoot(path)
	if err != nil {
		t.Fatal(err)
	}
	store, err := OpenBound(context.Background(), root)
	if err != nil {
		_ = root.Close()
		t.Fatal(err)
	}
	return store, root
}

func testID(prefix string, serial int) string {
	return fmt.Sprintf("%s_%026d", prefix, serial)
}

func canonicalCommand(t *testing.T, version int64, serial int, key, operation string) []byte {
	t.Helper()
	commandID, messageID := testID("cmd", serial), testID("msg", serial)
	command := core.CommandEnvelope{
		ActorRef:         core.ActorRef{ActorID: testID("acr", 1), ActorType: "service"},
		Canonicalization: core.CanonicalizationV1, CommandID: commandID,
		CorrelationID: testID("cor", serial), EnvelopeVersion: core.EnvelopeVersionV1,
		ExpectedVersion: &version, Extensions: map[string]any{}, IdempotencyKey: key,
		IssuedAtUnixMS: testUnixMS, MessageID: messageID,
		Payload: map[string]any{"operation": operation}, SchemaName: "forge.control.test_command",
		SchemaVersion: 1, ScopeRef: core.ScopeRef{SpaceID: testID("spc", 1)},
		TargetRef: core.EntityRef{EntityID: testID("spc", 1), EntityType: "space"},
	}
	body, err := core.CanonicalCommandEnvelopeJSON(&command)
	if err != nil {
		t.Fatal(err)
	}
	return body
}

func canonicalEvent(
	t *testing.T,
	version, sequence int64,
	serial int,
	source, operation string,
) []byte {
	t.Helper()
	causation := testID("msg", serial)
	return canonicalEventWithLinks(
		t, version, sequence, serial, source, operation, testID("cor", serial), &causation,
	)
}

func canonicalEventWithLinks(
	t *testing.T,
	version, sequence int64,
	serial int,
	source, operation, correlation string,
	causation *string,
) []byte {
	t.Helper()
	eventSerial := serial + 1000
	event := core.EventEnvelope{
		ActorRef:         core.ActorRef{ActorID: testID("acr", 1), ActorType: "service"},
		AggregateRef:     core.EntityRef{EntityID: testID("spc", 1), EntityType: "space"},
		AggregateVersion: version, Canonicalization: core.CanonicalizationV1,
		CausationID: causation, CorrelationID: correlation,
		EnvelopeVersion: core.EnvelopeVersionV1, EventID: testID("evt", eventSerial),
		Extensions: map[string]any{}, MessageID: testID("msg", eventSerial),
		OccurredAtUnixMS: testUnixMS + sequence, Payload: map[string]any{"operation": operation},
		SchemaName: "forge.control.test_event", SchemaVersion: 1,
		ScopeRef: core.ScopeRef{SpaceID: testID("spc", 1)}, Sequence: sequence,
		SourceComponent: core.SourceComponent(source),
	}
	body, err := core.CanonicalEventEnvelopeJSON(&event)
	if err != nil {
		t.Fatal(err)
	}
	return body
}

func commitRequest(t *testing.T, version, sequence int64, serial int) CommitRequest {
	t.Helper()
	key := fmt.Sprintf("control-command-%04d", serial)
	return CommitRequest{
		Command: canonicalCommand(t, version, serial, key, "advance"),
		Events:  [][]byte{canonicalEvent(t, version+1, sequence, serial, "control_plane", "advanced")},
		Result:  []byte(fmt.Sprintf(`{"accepted":%d}`, serial)), CommittedAtUnixMS: testUnixMS,
	}
}

func mutateCanonicalEvent(
	t *testing.T,
	body []byte,
	mutate func(*core.EventEnvelope),
) []byte {
	t.Helper()
	event, err := core.DecodeCanonicalEventEnvelope(body)
	if err != nil {
		t.Fatal(err)
	}
	mutate(event)
	result, err := core.CanonicalEventEnvelopeJSON(event)
	if err != nil {
		t.Fatal(err)
	}
	return result
}

func mutateCanonicalCommand(
	t *testing.T,
	body []byte,
	mutate func(*core.CommandEnvelope),
) []byte {
	t.Helper()
	command, err := core.DecodeCanonicalCommandEnvelope(body)
	if err != nil {
		t.Fatal(err)
	}
	mutate(command)
	result, err := core.CanonicalCommandEnvelopeJSON(command)
	if err != nil {
		t.Fatal(err)
	}
	return result
}

func retargetCommit(t *testing.T, request CommitRequest, aggregateType string) CommitRequest {
	t.Helper()
	command, err := core.DecodeCanonicalCommandEnvelope(request.Command)
	if err != nil {
		t.Fatal(err)
	}
	event, err := core.DecodeCanonicalEventEnvelope(request.Events[0])
	if err != nil {
		t.Fatal(err)
	}
	scope, target := rustOwnedScope(aggregateType)
	command.ScopeRef, command.TargetRef = scope, target
	event.ScopeRef, event.AggregateRef = scope, target
	request.Command, err = core.CanonicalCommandEnvelopeJSON(command)
	if err != nil {
		t.Fatal(err)
	}
	request.Events[0], err = core.CanonicalEventEnvelopeJSON(event)
	if err != nil {
		t.Fatal(err)
	}
	return request
}

func rustOwnedScope(aggregateType string) (core.ScopeRef, core.EntityRef) {
	values := map[string]string{
		"project": testID("prj", 1), "objective": testID("obj", 1),
		"change": testID("chg", 1), "work_graph": testID("wgr", 1),
		"work_item": testID("wki", 1), "attempt": testID("atm", 1),
		"session": testID("ses", 1), "turn": testID("trn", 1), "action": testID("act", 1),
	}
	scope := core.ScopeRef{
		SpaceID: testID("spc", 1), ProjectID: stringPointer(values["project"]),
		ObjectiveID: stringPointer(values["objective"]), ChangeID: stringPointer(values["change"]),
		WorkGraphID: stringPointer(values["work_graph"]), WorkItemID: stringPointer(values["work_item"]),
		AttemptID: stringPointer(values["attempt"]), SessionID: stringPointer(values["session"]),
		TurnID: stringPointer(values["turn"]), ActionID: stringPointer(values["action"]),
	}
	return scope, core.EntityRef{EntityID: values[aggregateType], EntityType: core.EntityType(aggregateType)}
}

func stringPointer(value string) *string { return &value }
