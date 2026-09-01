package domain

import (
	"fmt"
	"reflect"
	"testing"

	core "forgeos/forge-core/internal/platformcorecontract"
)

const testUnixMS = int64(1788091200000)

func TestWorkspaceCreationFoldsExactEntities(t *testing.T) {
	space, err := FoldSpace([]*core.EventEnvelope{testSpaceEvent()})
	if err != nil || space.SpaceID != testID("spc", 1) || space.Name != "Local Factory" {
		t.Fatalf("Space = %+v, %v", space, err)
	}
	project, err := FoldProject([]*core.EventEnvelope{testProjectEvent()})
	if err != nil || project.SpaceID != space.SpaceID || project.RootPathStatus != RootPathDeclaredUnverified {
		t.Fatalf("Project = %+v, %v", project, err)
	}
	snapshot, err := FoldProjectSnapshot([]*core.EventEnvelope{testSnapshotEvent()})
	if err != nil || snapshot.ProjectID != project.ProjectID ||
		snapshot.ObservationStatus != ObservationDeclaredUnresolved {
		t.Fatalf("ProjectSnapshot = %+v, %v", snapshot, err)
	}
	if !reflect.DeepEqual(snapshot.ObservationRef, testObservationRef()) {
		t.Fatalf("observation ref = %+v", snapshot.ObservationRef)
	}
}

func TestSpaceFoldRejectsNonExactHistory(t *testing.T) {
	tests := []struct {
		name   string
		events func() []*core.EventEnvelope
	}{
		{"empty", func() []*core.EventEnvelope { return nil }},
		{"extra-event", func() []*core.EventEnvelope {
			return []*core.EventEnvelope{testSpaceEvent(), testSpaceEvent()}
		}},
		{"unknown-schema", func() []*core.EventEnvelope {
			event := testSpaceEvent()
			event.SchemaName = "forge.workspace.space_renamed"
			return []*core.EventEnvelope{event}
		}},
		{"invalid-actor", func() []*core.EventEnvelope {
			event := testSpaceEvent()
			event.ActorRef.ActorID = "bad"
			return []*core.EventEnvelope{event}
		}},
		{"extra-payload", func() []*core.EventEnvelope {
			event := testSpaceEvent()
			event.Payload["unexpected"] = true
			return []*core.EventEnvelope{event}
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if _, err := FoldSpace(test.events()); err == nil {
				t.Fatal("invalid Space history folded")
			}
		})
	}
}

func TestProjectFoldRejectsInvalidAliasAndDeclaredPath(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*core.EventEnvelope)
	}{
		{"uppercase-alias", func(event *core.EventEnvelope) { event.Payload["alias"] = "API" }},
		{"relative-path", func(event *core.EventEnvelope) { event.Payload["root_path"] = "repo" }},
		{"root-path", func(event *core.EventEnvelope) { event.Payload["root_path"] = "/" }},
		{"unclean-path", func(event *core.EventEnvelope) { event.Payload["root_path"] = "/tmp/../repo" }},
		{"verified-claim", func(event *core.EventEnvelope) { event.Payload["root_path_status"] = "verified" }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			event := testProjectEvent()
			test.mutate(event)
			if _, err := FoldProject([]*core.EventEnvelope{event}); err == nil {
				t.Fatal("invalid Project history folded")
			}
		})
	}
}

func TestSnapshotFoldRejectsUnresolvedReferenceDrift(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*core.EventEnvelope)
	}{
		{"future-capture", func(event *core.EventEnvelope) { event.Payload["captured_at_unix_ms"] = testUnixMS + 1 }},
		{"bad-record-hash", func(event *core.EventEnvelope) {
			event.Payload["observation_ref"].(map[string]any)["record_sha256"] = "bad"
		}},
		{"resolved-claim", func(event *core.EventEnvelope) { event.Payload["observation_status"] = "verified" }},
		{"wrong-source-ref", func(event *core.EventEnvelope) {
			event.SourceSnapshotRef = &core.EntityRef{EntityID: testID("psn", 9), EntityType: "project_snapshot"}
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			event := testSnapshotEvent()
			test.mutate(event)
			if _, err := FoldProjectSnapshot([]*core.EventEnvelope{event}); err == nil {
				t.Fatal("invalid Snapshot history folded")
			}
		})
	}
}

func testID(prefix string, serial int) string {
	return fmt.Sprintf("%s_%026d", prefix, serial)
}

func testActor() core.ActorRef {
	return core.ActorRef{ActorID: testID("acr", 1), ActorType: "human"}
}

func testObservationRef() core.RecordRef {
	return core.RecordRef{
		RecordID: "snapshot-observation-1", RecordSHA256: fmt.Sprintf("%064x", 1),
		RecordType: "forge.observation.project_snapshot",
	}
}
