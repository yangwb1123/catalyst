package domain

import core "forgeos/forge-core/internal/platformcorecontract"

func testSpaceEvent() *core.EventEnvelope {
	cause := testID("msg", 1)
	return &core.EventEnvelope{
		ActorRef: testActor(), AggregateRef: core.EntityRef{EntityID: testID("spc", 1), EntityType: "space"},
		AggregateVersion: 1, Canonicalization: core.CanonicalizationV1,
		CausationID: &cause, CorrelationID: testID("cor", 1), EnvelopeVersion: 1,
		EventID: testID("evt", 101), Extensions: map[string]any{}, MessageID: testID("msg", 101),
		OccurredAtUnixMS: testUnixMS, Payload: map[string]any{
			"name": "Local Factory", "operation": "create_space", "space_id": testID("spc", 1),
		},
		SchemaName: spaceCreatedSchema, SchemaVersion: 1,
		ScopeRef: core.ScopeRef{SpaceID: testID("spc", 1)}, Sequence: 1,
		SourceComponent: "control_plane",
	}
}

func testProjectEvent() *core.EventEnvelope {
	cause := testID("msg", 2)
	projectID := testID("prj", 1)
	return &core.EventEnvelope{
		ActorRef: testActor(), AggregateRef: core.EntityRef{EntityID: projectID, EntityType: "project"},
		AggregateVersion: 1, Canonicalization: core.CanonicalizationV1,
		CausationID: &cause, CorrelationID: testID("cor", 2), EnvelopeVersion: 1,
		EventID: testID("evt", 102), Extensions: map[string]any{}, MessageID: testID("msg", 102),
		OccurredAtUnixMS: testUnixMS, Payload: map[string]any{
			"alias": "api", "operation": "register_project", "project_id": projectID,
			"root_path": "/workspace/api", "root_path_status": RootPathDeclaredUnverified,
			"space_id": testID("spc", 1),
		},
		SchemaName: projectRegisteredSchema, SchemaVersion: 1,
		ScopeRef: core.ScopeRef{SpaceID: testID("spc", 1), ProjectID: testString(projectID)},
		Sequence: 2, SourceComponent: "control_plane",
	}
}

func testSnapshotEvent() *core.EventEnvelope {
	cause := testID("msg", 3)
	projectID, snapshotID := testID("prj", 1), testID("psn", 1)
	target := core.EntityRef{EntityID: snapshotID, EntityType: "project_snapshot"}
	observation := testObservationRef()
	return &core.EventEnvelope{
		ActorRef: testActor(), AggregateRef: target, AggregateVersion: 1,
		Canonicalization: core.CanonicalizationV1, CausationID: &cause,
		CorrelationID: testID("cor", 3), EnvelopeVersion: 1,
		EventID: testID("evt", 103), Extensions: map[string]any{}, MessageID: testID("msg", 103),
		OccurredAtUnixMS: testUnixMS, Payload: map[string]any{
			"captured_at_unix_ms": testUnixMS,
			"observation_ref": map[string]any{
				"record_id": observation.RecordID, "record_sha256": observation.RecordSHA256,
				"record_type": observation.RecordType,
			},
			"observation_status": ObservationDeclaredUnresolved,
			"operation":          "record_project_snapshot", "project_id": projectID,
			"project_snapshot_id": snapshotID, "space_id": testID("spc", 1),
		},
		SchemaName: snapshotRecordedSchema, SchemaVersion: 1,
		ScopeRef: core.ScopeRef{
			SpaceID: testID("spc", 1), ProjectID: testString(projectID),
			ProjectSnapshotID: testString(snapshotID),
		},
		Sequence: 3, SourceComponent: "control_plane", SourceSnapshotRef: &target,
	}
}

func testString(value string) *string { return &value }
