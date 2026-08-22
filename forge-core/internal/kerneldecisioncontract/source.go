package kerneldecisioncontract

import (
	"encoding/json"
	"fmt"
)

func rawString(member map[string]any, field, label string) (string, error) {
	value, ok := member[field].(string)
	if !ok {
		return "", fmt.Errorf("%s.%s must be text", label, field)
	}
	return value, nil
}

func validateAuthority(value DeclaredAuthority) error {
	if !oneOf(value.AuthorityKind, "approval_record", "architecture_decision",
		"contract_artifact", "none") {
		return fmt.Errorf("authority_kind is unsupported")
	}
	if value.AuthorityKind == "none" {
		if !rawIsNull(value.AuthorityRef) {
			return fmt.Errorf("none authority requires null authority_ref")
		}
		return nil
	}
	if rawIsNull(value.AuthorityRef) {
		return fmt.Errorf("declared authority reference is required")
	}
	if value.AuthorityKind == "approval_record" {
		return validateApprovalRef(value.AuthorityRef)
	}
	if value.AuthorityKind == "architecture_decision" {
		return validateADRRef(value.AuthorityRef)
	}
	return validateArtifactRaw(value.AuthorityRef, "authority_ref")
}

func validateApprovalRef(raw json.RawMessage) error {
	member, err := exactRawObject(raw, "approval_id", "approval_sha256", "authority_domain")
	if err != nil {
		return err
	}
	digest, err := rawString(member, "approval_sha256", "authority_ref")
	if err != nil || hash(digest, "approval_sha256") != nil {
		return fmt.Errorf("approval_sha256 is invalid")
	}
	id, err := rawString(member, "approval_id", "authority_ref")
	if err != nil || id != "approval-record-"+digest {
		return fmt.Errorf("approval_id does not bind approval_sha256")
	}
	domain, err := rawString(member, "authority_domain", "authority_ref")
	if err != nil {
		return err
	}
	return text(domain, "authority_domain", maxShortBytes)
}

func validateADRRef(raw json.RawMessage) error {
	member, err := exactRawObject(raw, "adr_id", "adr_self_sha256")
	if err != nil {
		return err
	}
	id, err := rawString(member, "adr_id", "authority_ref")
	if err != nil || !adrPattern.MatchString(id) {
		return fmt.Errorf("adr_id is invalid")
	}
	digest, err := rawString(member, "adr_self_sha256", "authority_ref")
	if err != nil {
		return err
	}
	return hash(digest, "adr_self_sha256")
}

func validateArtifactRaw(raw json.RawMessage, label string) error {
	member, err := exactRawObject(raw, "artifact_kind", "artifact_ref", "artifact_sha256")
	if err != nil {
		return err
	}
	kind, err := rawString(member, "artifact_kind", label)
	if err != nil {
		return err
	}
	reference, err := rawString(member, "artifact_ref", label)
	if err != nil {
		return err
	}
	digest, err := rawString(member, "artifact_sha256", label)
	if err != nil || hash(digest, label+".artifact_sha256") != nil {
		return fmt.Errorf("%s artifact digest is invalid", label)
	}
	if err := text(kind, label+".artifact_kind", maxShortBytes); err != nil {
		return err
	}
	return text(reference, label+".artifact_ref", maxSelectorBytes)
}

func validateSource(value AtomSource, atomType string) error {
	allowed, ok := sourceTypes[value.SourceKind]
	if !ok || !allowed[atomType] {
		return fmt.Errorf("source_kind does not admit atom_type")
	}
	pre := oneOf(value.SourceKind, "artifact", "cognitive_atom_v1", "evidence_record", "work_intent")
	expected := "postdecision"
	if pre {
		expected = "predecision"
	}
	if value.SourcePhase != expected {
		return fmt.Errorf("source_phase does not match source_kind")
	}
	if err := validateSourceRef(value.SourceKind, value.SourceRef); err != nil {
		return err
	}
	if value.SourceSelector != nil {
		if err := validateJSONPointer(*value.SourceSelector); err != nil {
			return err
		}
	}
	if value.SourceKind == "cognitive_atom_v1" && value.SourceSelector != nil {
		return fmt.Errorf("cognitive_atom_v1 selector must be null")
	}
	return nil
}

func validateSourceRef(kind string, raw json.RawMessage) error {
	switch kind {
	case "artifact":
		return validateArtifactRaw(raw, "source_ref")
	case "cognitive_atom_v1":
		return validateLegacyRef(raw)
	case "evidence_record":
		return validateEvidenceRef(raw)
	case "work_intent":
		return validateBoundRef(raw, "work_intent_id", "work_intent_sha256", "work-intent-")
	case "artifact_receipt":
		return validateBoundRef(raw, "artifact_receipt_id", "artifact_receipt_sha256", "artifact-receipt-")
	case "capability_invocation":
		return validateBoundRef(raw, "invocation_id", "invocation_sha256", "capability-invocation-")
	case "interaction_event":
		return validateBoundRef(raw, "event_id", "event_sha256", "interaction-event-")
	default:
		return validateBoundRef(raw, "execution_receipt_id", "execution_receipt_sha256", "execution-receipt-")
	}
}

func validateBoundRef(raw json.RawMessage, idField, hashField, prefix string) error {
	member, err := exactRawObject(raw, idField, hashField)
	if err != nil {
		return err
	}
	digest, err := rawString(member, hashField, "source_ref")
	if err != nil || hash(digest, hashField) != nil {
		return fmt.Errorf("%s is invalid", hashField)
	}
	id, err := rawString(member, idField, "source_ref")
	if err != nil || id != prefix+digest {
		return fmt.Errorf("%s does not bind %s", idField, hashField)
	}
	return nil
}

func validateLegacyRef(raw json.RawMessage) error {
	member, err := exactRawObject(raw, "atom_id", "canonical_sha256")
	if err != nil {
		return err
	}
	id, err := rawString(member, "atom_id", "source_ref")
	if err != nil || !legacyAtomPattern.MatchString(id) {
		return fmt.Errorf("legacy atom_id is invalid")
	}
	digest, err := rawString(member, "canonical_sha256", "source_ref")
	if err != nil {
		return err
	}
	return hash(digest, "canonical_sha256")
}

func validateEvidenceRef(raw json.RawMessage) error {
	member, err := exactRawObject(raw, "canonical_sha256", "record_id")
	if err != nil {
		return err
	}
	digest, err := rawString(member, "canonical_sha256", "source_ref")
	if err != nil || hash(digest, "canonical_sha256") != nil {
		return fmt.Errorf("evidence canonical_sha256 is invalid")
	}
	record, err := rawString(member, "record_id", "source_ref")
	if err != nil {
		return err
	}
	return text(record, "record_id", maxShortBytes)
}

func validateJSONPointer(value string) error {
	if err := text(value, "source_selector", maxSelectorBytes); err != nil {
		return err
	}
	if value[0] != '/' {
		return fmt.Errorf("source_selector must be a nonempty JSON pointer")
	}
	for index := 0; index < len(value); index++ {
		if value[index] == '~' && (index+1 >= len(value) || value[index+1] != '0' && value[index+1] != '1') {
			return fmt.Errorf("source_selector has a noncanonical escape")
		}
		if value[index] == '~' {
			index++
		}
	}
	return nil
}
