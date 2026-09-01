package domain

import (
	"strings"
	"testing"

	core "forgeos/forge-core/internal/platformcorecontract"
)

func TestSpaceNameBoundaryAndUnicodeRules(t *testing.T) {
	for _, name := range []string{"x", strings.Repeat("a", 128), "本地工厂"} {
		event := testSpaceEvent()
		event.Payload["name"] = name
		if _, err := FoldSpace([]*core.EventEnvelope{event}); err != nil {
			t.Fatalf("valid name length %d: %v", len(name), err)
		}
	}
	invalid := []string{
		"", strings.Repeat("a", 129), " leading", "trailing ", "line\nbreak",
		"direction\u202eoverride", string([]byte{'x', 0xff}),
	}
	for _, name := range invalid {
		event := testSpaceEvent()
		event.Payload["name"] = name
		if _, err := FoldSpace([]*core.EventEnvelope{event}); err == nil {
			t.Fatalf("invalid name %q folded", name)
		}
	}
}

func TestProjectAliasBoundaryRules(t *testing.T) {
	for _, alias := range []string{"a", strings.Repeat("a", 64), "api.v1-local"} {
		event := testProjectEvent()
		event.Payload["alias"] = alias
		if _, err := FoldProject([]*core.EventEnvelope{event}); err != nil {
			t.Fatalf("valid alias %q: %v", alias, err)
		}
	}
	invalid := []string{
		"", strings.Repeat("a", 65), "-api", "api-", "API", "api/service", "服务",
	}
	for _, alias := range invalid {
		event := testProjectEvent()
		event.Payload["alias"] = alias
		if _, err := FoldProject([]*core.EventEnvelope{event}); err == nil {
			t.Fatalf("invalid alias %q folded", alias)
		}
	}
}

func TestProjectPathBoundaryAndDirectionalControlRules(t *testing.T) {
	for _, root := range []string{"/a", "/" + strings.Repeat("a", 4095)} {
		event := testProjectEvent()
		event.Payload["root_path"] = root
		if _, err := FoldProject([]*core.EventEnvelope{event}); err != nil {
			t.Fatalf("valid root length %d: %v", len(root), err)
		}
	}
	invalid := []string{
		"/", "relative", "/tmp/../repo", "/line\nbreak",
		"/" + strings.Repeat("a", 4096), "/" + string([]byte{0xff}),
	}
	for _, character := range []rune{
		'\u061c', '\u200e', '\u200f', '\u202a', '\u202b', '\u202c', '\u202d', '\u202e',
		'\u2066', '\u2067', '\u2068', '\u2069',
	} {
		invalid = append(invalid, "/workspace/"+string(character)+"repo")
	}
	for _, root := range invalid {
		event := testProjectEvent()
		event.Payload["root_path"] = root
		if _, err := FoldProject([]*core.EventEnvelope{event}); err == nil {
			t.Fatalf("invalid root %q folded", root)
		}
	}
}

func TestSnapshotTimestampBoundaryRules(t *testing.T) {
	for _, captured := range []int64{0, testUnixMS} {
		event := testSnapshotEvent()
		event.Payload["captured_at_unix_ms"] = captured
		if _, err := FoldProjectSnapshot([]*core.EventEnvelope{event}); err != nil {
			t.Fatalf("valid timestamp %d: %v", captured, err)
		}
	}
	event := testSnapshotEvent()
	event.OccurredAtUnixMS = maxUnixMilliseconds
	event.Payload["captured_at_unix_ms"] = maxUnixMilliseconds
	if _, err := FoldProjectSnapshot([]*core.EventEnvelope{event}); err != nil {
		t.Fatalf("maximum timestamp: %v", err)
	}
	for _, captured := range []int64{-1, maxUnixMilliseconds + 1, testUnixMS + 1} {
		event := testSnapshotEvent()
		event.Payload["captured_at_unix_ms"] = captured
		if _, err := FoldProjectSnapshot([]*core.EventEnvelope{event}); err == nil {
			t.Fatalf("invalid timestamp %d folded", captured)
		}
	}
}

func TestSnapshotNestedRecordReferenceRules(t *testing.T) {
	mutations := []func(map[string]any){
		func(value map[string]any) { value["record_id"] = "" },
		func(value map[string]any) { value["record_sha256"] = strings.Repeat("0", 63) },
		func(value map[string]any) { value["record_type"] = "Bad Type" },
		func(value map[string]any) { value["extra"] = true },
	}
	for index, mutate := range mutations {
		event := testSnapshotEvent()
		observation := event.Payload["observation_ref"].(map[string]any)
		mutate(observation)
		if _, err := FoldProjectSnapshot([]*core.EventEnvelope{event}); err == nil {
			t.Fatalf("invalid observation mutation %d folded", index)
		}
	}
}

func TestCreationEnvelopeMutationRules(t *testing.T) {
	mutations := []struct {
		name   string
		mutate func(*core.EventEnvelope)
	}{
		{"aggregate-id-type", func(event *core.EventEnvelope) {
			event.AggregateRef = core.EntityRef{EntityID: testID("prj", 9), EntityType: "project"}
		}},
		{"aggregate-version", func(event *core.EventEnvelope) { event.AggregateVersion = 2 }},
		{"schema-version", func(event *core.EventEnvelope) { event.SchemaVersion = 2 }},
		{"missing-space-scope", func(event *core.EventEnvelope) { event.ScopeRef.SpaceID = "" }},
		{"extra-project-scope", func(event *core.EventEnvelope) {
			event.ScopeRef.ProjectID = testString(testID("prj", 9))
		}},
		{"valid-wrong-source", func(event *core.EventEnvelope) { event.SourceComponent = "runtime" }},
		{"name-type", func(event *core.EventEnvelope) { event.Payload["name"] = int64(1) }},
		{"space-id-type", func(event *core.EventEnvelope) { event.Payload["space_id"] = true }},
	}
	for _, test := range mutations {
		t.Run(test.name, func(t *testing.T) {
			event := testSpaceEvent()
			test.mutate(event)
			if _, err := FoldSpace([]*core.EventEnvelope{event}); err == nil {
				t.Fatal("mutated creation envelope folded")
			}
		})
	}
}

func TestProjectAndSnapshotPayloadTypeDrift(t *testing.T) {
	project := testProjectEvent()
	project.Payload["root_path"] = int64(1)
	if _, err := FoldProject([]*core.EventEnvelope{project}); err == nil {
		t.Fatal("numeric Project path folded")
	}
	project = testProjectEvent()
	project.ScopeRef.ObjectiveID = testString(testID("obj", 1))
	if _, err := FoldProject([]*core.EventEnvelope{project}); err == nil {
		t.Fatal("extra Project scope folded")
	}
	snapshot := testSnapshotEvent()
	snapshot.Payload["observation_ref"] = "not-an-object"
	if _, err := FoldProjectSnapshot([]*core.EventEnvelope{snapshot}); err == nil {
		t.Fatal("text Snapshot observation folded")
	}
	snapshot = testSnapshotEvent()
	snapshot.Payload["captured_at_unix_ms"] = "now"
	if _, err := FoldProjectSnapshot([]*core.EventEnvelope{snapshot}); err == nil {
		t.Fatal("text Snapshot timestamp folded")
	}
}
