package platformcorecontract

type EntityType string
type ActorType string
type SourceComponent string

type EntityRef struct {
	EntityID   string     `json:"entity_id"`
	EntityType EntityType `json:"entity_type"`
}

type ActorRef struct {
	ActorID   string    `json:"actor_id"`
	ActorType ActorType `json:"actor_type"`
}

type RecordRef struct {
	RecordID     string `json:"record_id"`
	RecordSHA256 string `json:"record_sha256"`
	RecordType   string `json:"record_type"`
}

type ScopeRef struct {
	ActionID          *string `json:"action_id"`
	AttemptID         *string `json:"attempt_id"`
	ChangeID          *string `json:"change_id"`
	ObjectiveID       *string `json:"objective_id"`
	ProjectID         *string `json:"project_id"`
	ProjectSnapshotID *string `json:"project_snapshot_id"`
	SessionID         *string `json:"session_id"`
	SpaceID           string  `json:"space_id"`
	TurnID            *string `json:"turn_id"`
	WorkGraphID       *string `json:"work_graph_id"`
	WorkItemID        *string `json:"work_item_id"`
}

type ArtifactRef struct {
	ArtifactKind      string    `json:"artifact_kind"`
	Canonicalization  string    `json:"canonicalization"`
	ContentDigest     string    `json:"content_digest"`
	ContentID         string    `json:"content_id"`
	CreatedAtUnixMS   int64     `json:"created_at_unix_ms"`
	LogicalID         string    `json:"logical_id"`
	MediaType         string    `json:"media_type"`
	ProducerAttemptID string    `json:"producer_attempt_id"`
	ProvenanceRef     RecordRef `json:"provenance_ref"`
	RetentionClass    string    `json:"retention_class"`
	Sensitivity       string    `json:"sensitivity"`
	SizeBytes         int64     `json:"size_bytes"`
	SourceSnapshotRef EntityRef `json:"source_snapshot_ref"`
}

type CommandEnvelope struct {
	ActorRef           ActorRef       `json:"actor_ref"`
	AuthorizationRef   *RecordRef     `json:"authorization_ref"`
	Canonicalization   string         `json:"canonicalization"`
	CausationID        *string        `json:"causation_id"`
	CommandID          string         `json:"command_id"`
	CorrelationID      string         `json:"correlation_id"`
	DeadlineUnixMS     *int64         `json:"deadline_unix_ms"`
	EnvelopeVersion    int64          `json:"envelope_version"`
	ExpectedVersion    *int64         `json:"expected_version"`
	Extensions         map[string]any `json:"extensions"`
	IdempotencyKey     string         `json:"idempotency_key"`
	IssuedAtUnixMS     int64          `json:"issued_at_unix_ms"`
	MessageID          string         `json:"message_id"`
	Payload            map[string]any `json:"payload"`
	PayloadArtifactRef *ArtifactRef   `json:"payload_artifact_ref"`
	SchemaName         string         `json:"schema_name"`
	SchemaVersion      int64          `json:"schema_version"`
	ScopeRef           ScopeRef       `json:"scope_ref"`
	TargetRef          EntityRef      `json:"target_ref"`
}

type EventEnvelope struct {
	ActorRef           ActorRef        `json:"actor_ref"`
	AggregateRef       EntityRef       `json:"aggregate_ref"`
	AggregateVersion   int64           `json:"aggregate_version"`
	Canonicalization   string          `json:"canonicalization"`
	CausationID        *string         `json:"causation_id"`
	CorrelationID      string          `json:"correlation_id"`
	EnvelopeVersion    int64           `json:"envelope_version"`
	EventID            string          `json:"event_id"`
	Extensions         map[string]any  `json:"extensions"`
	MessageID          string          `json:"message_id"`
	OccurredAtUnixMS   int64           `json:"occurred_at_unix_ms"`
	Payload            map[string]any  `json:"payload"`
	PayloadArtifactRef *ArtifactRef    `json:"payload_artifact_ref"`
	SchemaName         string          `json:"schema_name"`
	SchemaVersion      int64           `json:"schema_version"`
	ScopeRef           ScopeRef        `json:"scope_ref"`
	Sequence           int64           `json:"sequence"`
	SourceComponent    SourceComponent `json:"source_component"`
	SourceSnapshotRef  *EntityRef      `json:"source_snapshot_ref"`
}
