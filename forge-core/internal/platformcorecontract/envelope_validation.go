package platformcorecontract

import (
	"fmt"
	"strings"
)

type envelopeValidationView struct {
	canonicalization string
	causationID      *string
	correlationID    string
	envelopeVersion  int64
	extensions       map[string]any
	messageID        string
	payload          map[string]any
	payloadArtifact  *ArtifactRef
	schemaName       string
	schemaVersion    int64
	scope            ScopeRef
	actor            ActorRef
}

func validateEnvelopeValues(view envelopeValidationView) error {
	if view.extensions == nil {
		return reject(rejectionDocumentInvalid, "extensions must be an object, not null")
	}
	if view.canonicalization != CanonicalizationV1 || view.envelopeVersion != EnvelopeVersionV1 {
		return fmt.Errorf("envelope canonicalization or envelope_version is unsupported")
	}
	if view.schemaVersion < 1 {
		return fmt.Errorf("schema_version must be positive")
	}
	if err := validateEnvelopeIDs(view); err != nil {
		return err
	}
	if err := validateSchemaName(view.schemaName, "schema_name"); err != nil {
		return err
	}
	if err := validateActorRef(view.actor); err != nil {
		return err
	}
	if err := validateScopeValues(view.scope); err != nil {
		return err
	}
	return validateEnvelopeBodyValues(view)
}

func validateEnvelopeIDs(view envelopeValidationView) error {
	if err := validateTypedID(view.messageID, "msg", "message_id"); err != nil {
		return err
	}
	if err := validateTypedID(view.correlationID, "cor", "correlation_id"); err != nil {
		return err
	}
	if view.causationID != nil {
		return validateTypedID(*view.causationID, "msg", "causation_id")
	}
	return nil
}

func validateEnvelopeBodyValues(view envelopeValidationView) error {
	if view.payload != nil {
		if _, err := canonicalJSON(view.payload, maxPayloadBytes); err != nil {
			return fmt.Errorf("payload: %w", err)
		}
	}
	if view.payloadArtifact != nil {
		if err := validateArtifactValues(view.payloadArtifact); err != nil {
			return fmt.Errorf("payload_artifact_ref: %w", err)
		}
	}
	return validateExtensions(view.extensions)
}

func validateEnvelopeReferences(view envelopeValidationView) error {
	if err := validateScopeAncestry(view.scope); err != nil {
		return err
	}
	if view.payloadArtifact == nil {
		return nil
	}
	if err := validateArtifactReferences(view.payloadArtifact); err != nil {
		return err
	}
	if view.scope.ProjectSnapshotID == nil ||
		view.payloadArtifact.SourceSnapshotRef.EntityID != *view.scope.ProjectSnapshotID {
		return reject(rejectionReferenceMismatch, "payload_artifact_ref source snapshot must match scope")
	}
	if view.scope.AttemptID == nil || view.payloadArtifact.ProducerAttemptID != *view.scope.AttemptID {
		return reject(rejectionReferenceMismatch, "payload_artifact_ref producer attempt must match scope")
	}
	return nil
}

func validateEnvelopeRelations(view envelopeValidationView) error {
	if view.causationID != nil && *view.causationID == view.messageID {
		return reject(rejectionRelationMismatch, "causation_id must not equal message_id")
	}
	if (view.payload == nil) == (view.payloadArtifact == nil) {
		return reject(rejectionRelationMismatch, "exactly one of payload or payload_artifact_ref is required")
	}
	if view.payloadArtifact != nil {
		return validateArtifactRelations(view.payloadArtifact)
	}
	return nil
}

func validateExtensions(value map[string]any) error {
	if len(value) > maxExtensionFields {
		return fmt.Errorf("extensions exceeds %d fields", maxExtensionFields)
	}
	for key := range value {
		if err := validateExtensionKey(key); err != nil {
			return err
		}
	}
	if _, err := canonicalJSON(value, maxExtensionsBytes); err != nil {
		return fmt.Errorf("extensions: %w", err)
	}
	return nil
}

func validateExtensionKey(value string) error {
	parts := strings.Split(value, ".")
	if len(parts) != 2 || len(parts[0]) < 2 || len(parts[0]) > 32 || len(parts[1]) > 64 {
		return fmt.Errorf("extension key %q must have two bounded namespace segments", value)
	}
	for _, part := range parts {
		if err := validateLowerToken(part, "extension key", 64); err != nil {
			return err
		}
	}
	return nil
}

func validateIdempotencyKey(value string) error {
	if len(value) < 16 || len(value) > 128 {
		return fmt.Errorf("idempotency_key must contain 16..128 visible ASCII bytes")
	}
	for _, character := range []byte(value) {
		if character < '!' || character > '~' {
			return fmt.Errorf("idempotency_key must contain visible ASCII only")
		}
	}
	return nil
}
