package platformcorecontract

// ValidateCommandEnvelope validates one supplied command without authorizing it.
func ValidateCommandEnvelope(value *CommandEnvelope) error {
	_, err := CanonicalCommandEnvelopeJSON(value)
	return err
}

// CanonicalCommandEnvelopeJSON returns exact compact canonical v1 bytes.
func CanonicalCommandEnvelopeJSON(value *CommandEnvelope) ([]byte, error) {
	if err := validateCommandFields(value); err != nil {
		return nil, withRejection(err, rejectionValueInvalid)
	}
	canonical, err := typedCanonical(value, maxEnvelopeBytes)
	return canonical, withRejection(err, rejectionValueInvalid)
}

// DecodeCanonicalCommandEnvelope accepts only one exact canonical v1 command.
func DecodeCanonicalCommandEnvelope(data []byte) (*CommandEnvelope, error) {
	var value CommandEnvelope
	if err := decodeTypedCanonical(data, maxEnvelopeBytes, &value); err != nil {
		return nil, withRejection(err, rejectionDocumentInvalid)
	}
	if err := validateExactTypedDocument(data, &value, maxEnvelopeBytes, "CommandEnvelope"); err != nil {
		return nil, err
	}
	if err := validateCommandFields(&value); err != nil {
		return nil, withRejection(err, rejectionValueInvalid)
	}
	return &value, nil
}

// CommandEnvelopeSHA256 returns a domain-separated conformance digest.
func CommandEnvelopeSHA256(value *CommandEnvelope) (string, error) {
	canonical, err := CanonicalCommandEnvelopeJSON(value)
	if err != nil {
		return "", err
	}
	return observationDigest(commandDigestDomain, canonical), nil
}

func validateCommandFields(value *CommandEnvelope) error {
	if value == nil {
		return reject(rejectionDocumentInvalid, "CommandEnvelope is required")
	}
	view := commandEnvelopeValidationView(value)
	if err := validateEnvelopeValues(view); err != nil {
		return err
	}
	if err := validateCommandValues(value); err != nil {
		return err
	}
	if err := validateEnvelopeReferences(view); err != nil {
		return err
	}
	if !scopeContains(value.ScopeRef, value.TargetRef) {
		return reject(rejectionReferenceMismatch, "target_ref is not represented by scope_ref")
	}
	if err := validateEnvelopeRelations(view); err != nil {
		return err
	}
	return validateCommandRelations(value)
}

func validateCommandValues(value *CommandEnvelope) error {
	if err := validateTypedID(value.CommandID, "cmd", "command_id"); err != nil {
		return err
	}
	if err := validateEntityRef(value.TargetRef, "target_ref"); err != nil {
		return err
	}
	if err := validateUnixMS(value.IssuedAtUnixMS, "issued_at_unix_ms"); err != nil {
		return err
	}
	if value.DeadlineUnixMS != nil {
		if err := validateUnixMS(*value.DeadlineUnixMS, "deadline_unix_ms"); err != nil {
			return err
		}
	}
	if value.ExpectedVersion != nil && *value.ExpectedVersion < 0 {
		return reject(rejectionValueInvalid, "expected_version must be null or nonnegative")
	}
	if err := validateIdempotencyKey(value.IdempotencyKey); err != nil {
		return err
	}
	if value.AuthorizationRef != nil {
		return validateRecordRef(*value.AuthorizationRef, "authorization_ref")
	}
	return nil
}

func validateCommandRelations(value *CommandEnvelope) error {
	if !sameMessageSuffix(value.MessageID, value.CommandID) {
		return reject(rejectionRelationMismatch, "message_id and command_id suffixes must match")
	}
	if value.DeadlineUnixMS != nil && *value.DeadlineUnixMS < value.IssuedAtUnixMS {
		return reject(rejectionRelationMismatch, "deadline_unix_ms must not precede issued_at_unix_ms")
	}
	if value.PayloadArtifactRef != nil &&
		value.PayloadArtifactRef.CreatedAtUnixMS > value.IssuedAtUnixMS {
		return reject(rejectionRelationMismatch, "command payload artifact cannot be created after issue time")
	}
	return nil
}

func commandEnvelopeValidationView(value *CommandEnvelope) envelopeValidationView {
	return envelopeValidationView{
		canonicalization: value.Canonicalization, causationID: value.CausationID,
		correlationID: value.CorrelationID, envelopeVersion: value.EnvelopeVersion,
		extensions: value.Extensions, messageID: value.MessageID, payload: value.Payload,
		payloadArtifact: value.PayloadArtifactRef, schemaName: value.SchemaName,
		schemaVersion: value.SchemaVersion, scope: value.ScopeRef, actor: value.ActorRef,
	}
}
