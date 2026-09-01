package platformcorecontract

// ValidateEventEnvelope validates one supplied event without appending it.
func ValidateEventEnvelope(value *EventEnvelope) error {
	_, err := CanonicalEventEnvelopeJSON(value)
	return err
}

// CanonicalEventEnvelopeJSON returns exact compact canonical v1 bytes.
func CanonicalEventEnvelopeJSON(value *EventEnvelope) ([]byte, error) {
	if err := validateEventFields(value); err != nil {
		return nil, withRejection(err, rejectionValueInvalid)
	}
	canonical, err := typedCanonical(value, maxEnvelopeBytes)
	return canonical, withRejection(err, rejectionValueInvalid)
}

// DecodeCanonicalEventEnvelope accepts only one exact canonical v1 event.
func DecodeCanonicalEventEnvelope(data []byte) (*EventEnvelope, error) {
	var value EventEnvelope
	if err := decodeTypedCanonical(data, maxEnvelopeBytes, &value); err != nil {
		return nil, withRejection(err, rejectionDocumentInvalid)
	}
	if err := validateExactTypedDocument(data, &value, maxEnvelopeBytes, "EventEnvelope"); err != nil {
		return nil, err
	}
	if err := validateEventFields(&value); err != nil {
		return nil, withRejection(err, rejectionValueInvalid)
	}
	return &value, nil
}

// EventEnvelopeSHA256 returns a domain-separated conformance digest.
func EventEnvelopeSHA256(value *EventEnvelope) (string, error) {
	canonical, err := CanonicalEventEnvelopeJSON(value)
	if err != nil {
		return "", err
	}
	return observationDigest(eventDigestDomain, canonical), nil
}

func validateEventFields(value *EventEnvelope) error {
	if value == nil {
		return reject(rejectionDocumentInvalid, "EventEnvelope is required")
	}
	view := eventEnvelopeValidationView(value)
	if err := validateEnvelopeValues(view); err != nil {
		return err
	}
	if err := validateEventValues(value); err != nil {
		return err
	}
	if err := validateEnvelopeReferences(view); err != nil {
		return err
	}
	if err := validateEventReferences(value); err != nil {
		return err
	}
	if err := validateEnvelopeRelations(view); err != nil {
		return err
	}
	return validateEventRelations(value)
}

func validateEventValues(value *EventEnvelope) error {
	if err := validateTypedID(value.EventID, "evt", "event_id"); err != nil {
		return err
	}
	if err := validateEntityRef(value.AggregateRef, "aggregate_ref"); err != nil {
		return err
	}
	if value.AggregateVersion < 1 || value.Sequence < 1 {
		return reject(rejectionValueInvalid, "aggregate_version and sequence must be positive")
	}
	if err := validateUnixMS(value.OccurredAtUnixMS, "occurred_at_unix_ms"); err != nil {
		return err
	}
	if _, err := parseSourceComponent(string(value.SourceComponent)); err != nil {
		return err
	}
	if value.SourceSnapshotRef != nil {
		return validateEntityRef(*value.SourceSnapshotRef, "source_snapshot_ref")
	}
	return nil
}

func validateEventReferences(value *EventEnvelope) error {
	if !scopeContains(value.ScopeRef, value.AggregateRef) {
		return reject(rejectionReferenceMismatch, "aggregate_ref is not represented by scope_ref")
	}
	scoped := value.ScopeRef.ProjectSnapshotID
	if scoped == nil && value.SourceSnapshotRef == nil {
		return nil
	}
	if scoped == nil || value.SourceSnapshotRef == nil ||
		value.SourceSnapshotRef.EntityType != entityProjectSnapshot ||
		value.SourceSnapshotRef.EntityID != *scoped {
		return reject(rejectionReferenceMismatch, "source_snapshot_ref must exactly match scoped project_snapshot_id")
	}
	return nil
}

func validateEventRelations(value *EventEnvelope) error {
	if !sameMessageSuffix(value.MessageID, value.EventID) {
		return reject(rejectionRelationMismatch, "message_id and event_id suffixes must match")
	}
	if value.PayloadArtifactRef != nil &&
		value.PayloadArtifactRef.CreatedAtUnixMS > value.OccurredAtUnixMS {
		return reject(rejectionRelationMismatch, "event payload artifact cannot be created after occurrence time")
	}
	return nil
}

func eventEnvelopeValidationView(value *EventEnvelope) envelopeValidationView {
	return envelopeValidationView{
		canonicalization: value.Canonicalization, causationID: value.CausationID,
		correlationID: value.CorrelationID, envelopeVersion: value.EnvelopeVersion,
		extensions: value.Extensions, messageID: value.MessageID, payload: value.Payload,
		payloadArtifact: value.PayloadArtifactRef, schemaName: value.SchemaName,
		schemaVersion: value.SchemaVersion, scope: value.ScopeRef, actor: value.ActorRef,
	}
}
