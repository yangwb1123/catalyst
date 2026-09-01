package platformcorecontract

import "testing"

func TestCommandSemanticRelationsFailClosed(t *testing.T) {
	cases := map[string]func(*CommandEnvelope){
		"message suffix": func(value *CommandEnvelope) { value.CommandID = "cmd_0000000000000000000000000f" },
		"self causation": func(value *CommandEnvelope) { cause := value.MessageID; value.CausationID = &cause },
		"target type":    func(value *CommandEnvelope) { value.TargetRef.EntityType = entityAttempt },
		"target outside": func(value *CommandEnvelope) { value.TargetRef.EntityID = "wki_0000000000000000000000000f" },
		"deadline":       func(value *CommandEnvelope) { deadline := value.IssuedAtUnixMS - 1; value.DeadlineUnixMS = &deadline },
		"version":        func(value *CommandEnvelope) { version := int64(-1); value.ExpectedVersion = &version },
		"key":            func(value *CommandEnvelope) { value.IdempotencyKey = "short" },
		"authorization":  func(value *CommandEnvelope) { value.AuthorizationRef.RecordSHA256 = "bad" },
		"both payloads": func(value *CommandEnvelope) {
			artifact := loadGoldenFixture(t).ArtifactRef
			value.PayloadArtifactRef = &artifact
		},
		"no payload":      func(value *CommandEnvelope) { value.Payload = nil },
		"null extensions": func(value *CommandEnvelope) { value.Extensions = nil },
	}
	for name, mutate := range cases {
		t.Run(name, func(t *testing.T) {
			value := loadGoldenFixture(t).CommandEnvelope
			mutate(&value)
			if err := ValidateCommandEnvelope(&value); err == nil {
				t.Fatal("expected semantic rejection")
			}
		})
	}
}

func TestEventSemanticRelationsFailClosed(t *testing.T) {
	cases := map[string]func(*EventEnvelope){
		"message suffix":    func(value *EventEnvelope) { value.EventID = "evt_0000000000000000000000000f" },
		"self causation":    func(value *EventEnvelope) { cause := value.MessageID; value.CausationID = &cause },
		"aggregate outside": func(value *EventEnvelope) { value.AggregateRef.EntityID = "atm_0000000000000000000000000f" },
		"aggregate version": func(value *EventEnvelope) { value.AggregateVersion = 0 },
		"sequence":          func(value *EventEnvelope) { value.Sequence = 0 },
		"source":            func(value *EventEnvelope) { value.SourceComponent = "unknown" },
		"snapshot missing":  func(value *EventEnvelope) { value.SourceSnapshotRef = nil },
		"snapshot drift":    func(value *EventEnvelope) { value.SourceSnapshotRef.EntityID = "psn_0000000000000000000000000f" },
		"future artifact":   func(value *EventEnvelope) { value.PayloadArtifactRef.CreatedAtUnixMS = value.OccurredAtUnixMS + 1 },
		"artifact snapshot": func(value *EventEnvelope) {
			value.PayloadArtifactRef.SourceSnapshotRef.EntityID = "psn_0000000000000000000000000f"
		},
		"artifact producer": func(value *EventEnvelope) {
			value.PayloadArtifactRef.ProducerAttemptID = "atm_0000000000000000000000000f"
		},
	}
	for name, mutate := range cases {
		t.Run(name, func(t *testing.T) {
			value := loadGoldenFixture(t).EventEnvelope
			mutate(&value)
			if err := ValidateEventEnvelope(&value); err == nil {
				t.Fatal("expected semantic rejection")
			}
		})
	}
}

func TestScopeRequiresContiguousAncestry(t *testing.T) {
	cases := map[string]func(*ScopeRef){
		"snapshot":  func(value *ScopeRef) { value.ProjectID = nil },
		"change":    func(value *ScopeRef) { value.ObjectiveID = nil },
		"graph":     func(value *ScopeRef) { value.ChangeID = nil },
		"work item": func(value *ScopeRef) { value.WorkGraphID = nil },
		"attempt":   func(value *ScopeRef) { value.WorkItemID = nil },
		"session":   func(value *ScopeRef) { value.AttemptID = nil },
		"turn":      func(value *ScopeRef) { value.SessionID = nil },
		"action":    func(value *ScopeRef) { value.TurnID = nil },
	}
	for name, mutate := range cases {
		t.Run(name, func(t *testing.T) {
			value := loadGoldenFixture(t).EventEnvelope.ScopeRef
			value.SessionID = testID("ses", "0c")
			value.TurnID = testID("trn", "0d")
			value.ActionID = testID("act", "0e")
			mutate(&value)
			if err := validateScope(value); err == nil {
				t.Fatal("expected ancestry rejection")
			}
		})
	}
}

func TestArtifactBackedCommandRelationsFailClosed(t *testing.T) {
	cases := map[string]func(*CommandEnvelope){
		"snapshot": func(value *CommandEnvelope) {
			value.PayloadArtifactRef.SourceSnapshotRef.EntityID = "psn_0000000000000000000000000f"
		},
		"producer": func(value *CommandEnvelope) {
			value.PayloadArtifactRef.ProducerAttemptID = "atm_0000000000000000000000000f"
		},
		"future": func(value *CommandEnvelope) {
			value.PayloadArtifactRef.CreatedAtUnixMS = value.IssuedAtUnixMS + 1
		},
	}
	for name, mutate := range cases {
		t.Run(name, func(t *testing.T) {
			value := artifactBackedCommand(t)
			mutate(&value)
			if err := ValidateCommandEnvelope(&value); err == nil {
				t.Fatal("expected artifact-backed command rejection")
			}
		})
	}
}

func artifactBackedCommand(t *testing.T) CommandEnvelope {
	fixture := loadGoldenFixture(t)
	value := fixture.CommandEnvelope
	artifact := fixture.ArtifactRef
	value.ScopeRef = fixture.EventEnvelope.ScopeRef
	value.Payload = nil
	value.PayloadArtifactRef = &artifact
	value.IssuedAtUnixMS = artifact.CreatedAtUnixMS
	deadline := value.IssuedAtUnixMS + 60_000
	value.DeadlineUnixMS = &deadline
	if err := ValidateCommandEnvelope(&value); err != nil {
		t.Fatalf("artifact-backed command fixture invalid: %v", err)
	}
	return value
}

func testID(prefix, ending string) *string {
	value := prefix + "_000000000000000000000000" + ending
	return &value
}

func TestPayloadSchemaVersionIsIndependentFromEnvelopeVersion(t *testing.T) {
	fixture := loadGoldenFixture(t)
	fixture.CommandEnvelope.SchemaVersion = 2
	fixture.EventEnvelope.SchemaVersion = 2
	if err := ValidateCommandEnvelope(&fixture.CommandEnvelope); err != nil {
		t.Fatalf("payload schema v2 command rejected: %v", err)
	}
	if err := ValidateEventEnvelope(&fixture.EventEnvelope); err != nil {
		t.Fatalf("payload schema v2 event rejected: %v", err)
	}
}

func TestWriterVocabularyParsersRejectUnknownValues(t *testing.T) {
	if _, err := parseEntityType("work_item"); err != nil {
		t.Fatal(err)
	}
	if _, err := parseActorType("service"); err != nil {
		t.Fatal(err)
	}
	if _, err := parseSourceComponent("runtime"); err != nil {
		t.Fatal(err)
	}
	if _, err := parseEntityType("run"); err == nil {
		t.Fatal("expected unknown entity rejection")
	}
	if _, err := parseActorType("admin"); err == nil {
		t.Fatal("expected unknown actor rejection")
	}
	if _, err := parseSourceComponent("worker"); err == nil {
		t.Fatal("expected unknown source rejection")
	}
}

func TestArtifactRelationsFailClosed(t *testing.T) {
	cases := map[string]func(*ArtifactRef){
		"content identity": func(value *ArtifactRef) { value.ContentID = "sha256:" + "0" + value.ContentDigest[1:] },
		"digest":           func(value *ArtifactRef) { value.ContentDigest = "bad" },
		"logical ID":       func(value *ArtifactRef) { value.LogicalID = "atm_0000000000000000000000000b" },
		"attempt ID":       func(value *ArtifactRef) { value.ProducerAttemptID = "art_00000000000000000000000008" },
		"snapshot type":    func(value *ArtifactRef) { value.SourceSnapshotRef.EntityType = entityProject },
		"provenance":       func(value *ArtifactRef) { value.ProvenanceRef.RecordType = "runtime" },
		"retention":        func(value *ArtifactRef) { value.RetentionClass = "forever" },
		"sensitivity":      func(value *ArtifactRef) { value.Sensitivity = "unknown" },
		"media type":       func(value *ArtifactRef) { value.MediaType = "application/json; charset=utf-8" },
		"media leading":    func(value *ArtifactRef) { value.MediaType = "application/+json" },
		"size":             func(value *ArtifactRef) { value.SizeBytes = maxArtifactBytes + 1 },
	}
	valid := loadGoldenFixture(t).ArtifactRef
	valid.MediaType = "application/vnd.forge+json"
	if err := ValidateArtifactRef(&valid); err != nil {
		t.Fatalf("valid media type rejected: %v", err)
	}
	for name, mutate := range cases {
		t.Run(name, func(t *testing.T) {
			value := loadGoldenFixture(t).ArtifactRef
			mutate(&value)
			if err := ValidateArtifactRef(&value); err == nil {
				t.Fatal("expected ArtifactRef rejection")
			}
		})
	}
}
