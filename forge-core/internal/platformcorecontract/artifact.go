package platformcorecontract

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"strings"
)

// ValidateArtifactRef validates supplied ArtifactRef declarations only. Composed
// contracts may request exactly one ordered stage: values, references, or relations.
func ValidateArtifactRef(value *ArtifactRef, stages ...string) error {
	if len(stages) == 0 {
		_, err := CanonicalArtifactRefJSON(value)
		return err
	}
	if len(stages) != 1 || value == nil {
		return reject(rejectionDocumentInvalid, "one non-nil staged ArtifactRef is required")
	}
	switch stages[0] {
	case "values":
		return withRejection(validateArtifactValues(value), rejectionValueInvalid)
	case "references":
		return validateArtifactReferences(value)
	case "relations":
		return validateArtifactRelations(value)
	default:
		return reject(rejectionValueInvalid, "ArtifactRef validation stage is unsupported")
	}
}

// CanonicalArtifactRefJSON returns exact compact canonical v1 bytes.
func CanonicalArtifactRefJSON(value *ArtifactRef) ([]byte, error) {
	if err := validateArtifactFields(value); err != nil {
		return nil, withRejection(err, rejectionValueInvalid)
	}
	canonical, err := typedCanonical(value, maxArtifactRefBytes)
	return canonical, withRejection(err, rejectionValueInvalid)
}

// DecodeCanonicalArtifactRef accepts only one exact canonical v1 ArtifactRef.
func DecodeCanonicalArtifactRef(data []byte) (*ArtifactRef, error) {
	var value ArtifactRef
	if err := decodeTypedCanonical(data, maxArtifactRefBytes, &value); err != nil {
		return nil, withRejection(err, rejectionDocumentInvalid)
	}
	if err := validateExactTypedDocument(data, &value, maxArtifactRefBytes, "ArtifactRef"); err != nil {
		return nil, err
	}
	if err := validateArtifactFields(&value); err != nil {
		return nil, withRejection(err, rejectionValueInvalid)
	}
	return &value, nil
}

// ArtifactRefSHA256 returns a domain-separated conformance observation digest.
func ArtifactRefSHA256(value *ArtifactRef) (string, error) {
	canonical, err := CanonicalArtifactRefJSON(value)
	if err != nil {
		return "", err
	}
	return observationDigest(artifactDigestDomain, canonical), nil
}

func validateArtifactFields(value *ArtifactRef) error {
	if value == nil {
		return reject(rejectionDocumentInvalid, "ArtifactRef is required")
	}
	if err := validateArtifactValues(value); err != nil {
		return err
	}
	if err := validateArtifactReferences(value); err != nil {
		return err
	}
	return validateArtifactRelations(value)
}

func validateArtifactValues(value *ArtifactRef) error {
	if value.Canonicalization != CanonicalizationV1 {
		return fmt.Errorf("ArtifactRef canonicalization is unsupported")
	}
	if err := validateLowerToken(value.ArtifactKind, "artifact_kind", 64); err != nil {
		return err
	}
	if err := validateHash(value.ContentDigest, "content_digest"); err != nil {
		return err
	}
	if err := validateTypedID(value.LogicalID, "art", "logical_id"); err != nil {
		return err
	}
	return validateArtifactDetails(value)
}

func validateArtifactDetails(value *ArtifactRef) error {
	if err := validateMediaType(value.MediaType); err != nil {
		return err
	}
	if err := validateTypedID(value.ProducerAttemptID, "atm", "producer_attempt_id"); err != nil {
		return err
	}
	if err := validateEntityRef(value.SourceSnapshotRef, "source_snapshot_ref"); err != nil {
		return err
	}
	if err := validateRecordRef(value.ProvenanceRef, "provenance_ref"); err != nil {
		return err
	}
	return validateArtifactClassification(value)
}

func validateArtifactReferences(value *ArtifactRef) error {
	if value.SourceSnapshotRef.EntityType != entityProjectSnapshot {
		return reject(rejectionReferenceMismatch, "source_snapshot_ref must reference project_snapshot")
	}
	return nil
}

func validateArtifactRelations(value *ArtifactRef) error {
	if value.ContentID != "sha256:"+value.ContentDigest {
		return reject(rejectionRelationMismatch, "content_id must equal sha256: plus content_digest")
	}
	return nil
}

func validateArtifactClassification(value *ArtifactRef) error {
	if err := validateUnixMS(value.CreatedAtUnixMS, "created_at_unix_ms"); err != nil {
		return err
	}
	if value.SizeBytes < 0 || value.SizeBytes > maxArtifactBytes {
		return fmt.Errorf("size_bytes must be in 0..%d", maxArtifactBytes)
	}
	if !oneOf(value.RetentionClass, "audit", "durable", "ephemeral", "legal_hold", "project") {
		return fmt.Errorf("retention_class is unsupported")
	}
	if !oneOf(value.Sensitivity, "confidential", "internal", "public", "secret") {
		return fmt.Errorf("sensitivity is unsupported")
	}
	return nil
}

func validateMediaType(value string) error {
	if err := validateText(value, "media_type", 128, true); err != nil {
		return err
	}
	parts := strings.Split(value, "/")
	if len(parts) != 2 || parts[0] == "" || parts[1] == "" {
		return fmt.Errorf("media_type must contain one type/subtype separator")
	}
	for _, part := range parts {
		if !lowerMediaTypeByte(part[0]) {
			return fmt.Errorf("media_type type and subtype must start with lowercase alphanumeric")
		}
		for _, character := range []byte(part) {
			if !mediaTypeByte(character) {
				return fmt.Errorf("media_type contains unsupported byte")
			}
		}
	}
	return nil
}

func lowerMediaTypeByte(value byte) bool {
	return value >= 'a' && value <= 'z' || value >= '0' && value <= '9'
}

func mediaTypeByte(value byte) bool {
	return value >= 'a' && value <= 'z' || value >= '0' && value <= '9' ||
		strings.ContainsRune("!#$&^_.+-", rune(value))
}

func observationDigest(domain string, canonical []byte) string {
	hasher := sha256.New()
	_, _ = hasher.Write([]byte(domain))
	_, _ = hasher.Write(canonical)
	return hex.EncodeToString(hasher.Sum(nil))
}

func oneOf(value string, options ...string) bool {
	for _, option := range options {
		if value == option {
			return true
		}
	}
	return false
}
