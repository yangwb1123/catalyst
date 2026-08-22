package workintentcontract

import (
	"bytes"
	"encoding/hex"
	"fmt"
	"regexp"
)

var adr0045Identifier = regexp.MustCompile(`^[a-z0-9][a-z0-9._:/-]{0,159}\z`)

func validateWorkIntentFields(value *WorkIntent, allowBlankIdentity bool) error {
	if value == nil {
		return fmt.Errorf("WorkIntent is nil")
	}
	if err := validateConstants(value); err != nil {
		return err
	}
	if value.DeclaredAtUnixMS < 0 {
		return fmt.Errorf("declared_at_unix_ms must be nonnegative")
	}
	if err := validateAttestations(value.Attestations); err != nil {
		return err
	}
	if err := validateBinding(value.Binding); err != nil {
		return err
	}
	if err := validatePrincipal(value.Requester, "requester"); err != nil {
		return err
	}
	if value.DeclaredOwner != nil {
		if err := validatePrincipal(*value.DeclaredOwner, "declared_owner"); err != nil {
			return err
		}
	}
	if err := validateIntent(value.Intent); err != nil {
		return err
	}
	if err := validateMateriality(value.Materiality); err != nil {
		return err
	}
	if err := validateOrigin(value.Origin); err != nil {
		return err
	}
	if err := validateReferences(value.References); err != nil {
		return err
	}
	return validateIdentityShape(value, allowBlankIdentity)
}

func validateConstants(value *WorkIntent) error {
	expected := []struct{ label, actual, wanted string }{
		{"api_version", value.APIVersion, APIVersion},
		{"canonicalization", value.Canonicalization, Canonicalization},
		{"freshness", value.Freshness, Freshness},
		{"kind", value.Kind, Kind},
		{"status", value.Status, Status},
	}
	for _, item := range expected {
		if item.actual != item.wanted {
			return fmt.Errorf("%s must be %q", item.label, item.wanted)
		}
	}
	return nil
}

func validateAttestations(value Attestations) error {
	values := []bool{value.Approval, value.Authentication, value.Authority,
		value.Completion, value.Effect, value.Execution, value.Freshness,
		value.Materiality, value.Ownership, value.Permission, value.Persistence,
		value.ReferenceResolution, value.Scope, value.Truth}
	for _, attestation := range values {
		if attestation {
			return fmt.Errorf("every WorkIntent attestation must be exactly false")
		}
	}
	return nil
}

func validateBinding(value Binding) error {
	if err := validateText(value.ChangeID, "binding.change_id", maxShortBytes); err != nil {
		return err
	}
	if err := validateText(value.ProjectID, "binding.project_id", maxShortBytes); err != nil {
		return err
	}
	if value.RunID != nil {
		return validateText(*value.RunID, "binding.run_id", maxShortBytes)
	}
	return nil
}

func validatePrincipal(value Principal, label string) error {
	if err := validateText(value.AuthorityDomain, label+".authority_domain", maxShortBytes); err != nil {
		return err
	}
	if err := validateText(value.PrincipalID, label+".principal_id", maxShortBytes); err != nil {
		return err
	}
	return validateEnum(value.PrincipalType, label+".principal_type",
		"agent", "human", "operator", "service")
}

func validateIntent(value Intent) error {
	if value.DeadlineUnixMS != nil && *value.DeadlineUnixMS < 0 {
		return fmt.Errorf("intent.deadline_unix_ms must be nonnegative or null")
	}
	if err := validateText(value.Goal, "intent.goal", maxStringBytes); err != nil {
		return err
	}
	lists := []struct {
		label   string
		values  []string
		minimum int
	}{
		{"intent.external_constraints", value.ExternalConstraints, 0},
		{"intent.non_goals", value.NonGoals, 0},
		{"intent.open_questions", value.OpenQuestions, 0},
		{"intent.scope", value.Scope, 1},
		{"intent.success_signals", value.SuccessSignals, 1},
	}
	total := 0
	for _, list := range lists {
		if err := validateNarrativeList(list.values, list.label, list.minimum); err != nil {
			return err
		}
		total += len(list.values)
	}
	if total > maxNarrativeTotal {
		return fmt.Errorf("intent narrative arrays exceed %d total items", maxNarrativeTotal)
	}
	return validateEnum(value.WorkType, "intent.work_type", "question", "defect",
		"small_change", "feature", "refactor", "migration", "incident_response",
		"architecture_evolution")
}

func validateNarrativeList(values []string, label string, minimum int) error {
	if values == nil || len(values) < minimum || len(values) > maxNarrativeItems {
		return fmt.Errorf("%s must contain %d..%d items", label, minimum, maxNarrativeItems)
	}
	seen := make(map[string]struct{}, len(values))
	for index, value := range values {
		if err := validateText(value, fmt.Sprintf("%s[%d]", label, index), maxStringBytes); err != nil {
			return err
		}
		if _, duplicate := seen[value]; duplicate {
			return fmt.Errorf("%s entries must be exact-string unique", label)
		}
		seen[value] = struct{}{}
	}
	return nil
}

func validateMateriality(value Materiality) error {
	if value.Basis != "caller_declaration_only" {
		return fmt.Errorf("materiality.basis must be caller_declaration_only")
	}
	return validateEnum(value.Level, "materiality.level",
		"materiality_not_bound", "L0", "L1", "L2", "L3", "L4")
}

func validateOrigin(value Origin) error {
	if err := validateEnum(value.OriginKind, "origin.origin_kind", "user_request",
		"runtime_signal", "incident", "technical_debt", "reflection_proposal",
		"operator_request", "other"); err != nil {
		return err
	}
	if value.OriginRef != nil {
		return validateText(*value.OriginRef, "origin.origin_ref", maxReferenceBytes)
	}
	return nil
}

func validateReferences(value References) error {
	if value.ClaimRecordRefs == nil || value.EvidenceRecordRefs == nil ||
		value.LocalArtifactDeclarations == nil {
		return fmt.Errorf("reference arrays must be present arrays, not null")
	}
	if len(value.ClaimRecordRefs)+len(value.EvidenceRecordRefs) > maxCombinedRefs {
		return fmt.Errorf("claim and evidence references exceed %d combined", maxCombinedRefs)
	}
	claims, err := validateRecordRefs(value.ClaimRecordRefs, "claim_record_refs")
	if err != nil {
		return err
	}
	evidence, err := validateRecordRefs(value.EvidenceRecordRefs, "evidence_record_refs")
	if err != nil {
		return err
	}
	for identifier := range claims {
		if _, overlap := evidence[identifier]; overlap {
			return fmt.Errorf("claim and evidence record IDs must be mutually disjoint")
		}
	}
	if err := validateArtifacts(value.LocalArtifactDeclarations); err != nil {
		return err
	}
	return validateSnapshot(value.LocalSourceSnapshotDeclaration)
}

func validateRecordRefs(values []RecordRef, label string) (map[string]struct{}, error) {
	if len(values) > maxRecordRefs {
		return nil, fmt.Errorf("%s exceeds %d items", label, maxRecordRefs)
	}
	seen := make(map[string]struct{}, len(values))
	var previous string
	for index, value := range values {
		if !adr0045Identifier.MatchString(value.RecordID) {
			return nil, fmt.Errorf("%s[%d].record_id violates ADR-0045 grammar", label, index)
		}
		if err := validateHash(value.CanonicalSHA256, label+".canonical_sha256"); err != nil {
			return nil, err
		}
		if index > 0 && bytes.Compare([]byte(previous), []byte(value.RecordID)) >= 0 {
			return nil, fmt.Errorf("%s must be strictly UTF-8 sorted and unique", label)
		}
		seen[value.RecordID] = struct{}{}
		previous = value.RecordID
	}
	return seen, nil
}

func validateArtifacts(values []ArtifactDeclaration) error {
	if len(values) > maxArtifactDecls {
		return fmt.Errorf("local_artifact_declarations exceeds %d items", maxArtifactDecls)
	}
	pairs := make(map[string]struct{}, len(values))
	var previous []byte
	for index, value := range values {
		if err := validateArtifact(value, index); err != nil {
			return err
		}
		pair := value.ArtifactKind + "\x00" + value.ArtifactRef
		if _, duplicate := pairs[pair]; duplicate {
			return fmt.Errorf("artifact kind/ref pairs must be unique")
		}
		pairs[pair] = struct{}{}
		encoded, err := canonicalArtifact(value)
		if err != nil {
			return err
		}
		if index > 0 && bytes.Compare(previous, encoded) >= 0 {
			return fmt.Errorf("artifacts must be strictly sorted by canonical member bytes")
		}
		previous = encoded
	}
	return nil
}

func validateArtifact(value ArtifactDeclaration, index int) error {
	prefix := fmt.Sprintf("local_artifact_declarations[%d]", index)
	if err := validateText(value.ArtifactKind, prefix+".artifact_kind", maxShortBytes); err != nil {
		return err
	}
	if err := validateText(value.ArtifactRef, prefix+".artifact_ref", maxReferenceBytes); err != nil {
		return err
	}
	return validateHash(value.ArtifactSHA256, prefix+".artifact_sha256")
}

func canonicalArtifact(value ArtifactDeclaration) ([]byte, error) {
	return canonicalJSON(map[string]any{
		"artifact_kind": value.ArtifactKind, "artifact_ref": value.ArtifactRef,
		"artifact_sha256": value.ArtifactSHA256,
	})
}

func validateSnapshot(value *SourceSnapshot) error {
	if value == nil {
		return nil
	}
	if !adr0045Identifier.MatchString(value.SnapshotID) {
		return fmt.Errorf("snapshot_id violates ADR-0045 grammar")
	}
	if err := validateHash(value.SnapshotSHA256, "snapshot_sha256"); err != nil {
		return err
	}
	return validateEnum(value.SnapshotType, "snapshot_type",
		"artifact", "external", "repository", "runtime")
}

func validateIdentityShape(value *WorkIntent, allowBlank bool) error {
	if allowBlank && value.WorkIntentID == "" && value.WorkIntentSHA256 == "" {
		return nil
	}
	if err := validateHash(value.WorkIntentSHA256, "work_intent_sha256"); err != nil {
		return err
	}
	if value.WorkIntentID != workIntentIDPrefix+value.WorkIntentSHA256 {
		return fmt.Errorf("work_intent_id must bind work_intent_sha256")
	}
	return nil
}

func validateText(value, label string, maximum int) error {
	if value == "" || len(value) > maximum {
		return fmt.Errorf("%s byte length must be 1..%d", label, maximum)
	}
	if err := validateWireString(value); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	return nil
}

func validateHash(value, label string) error {
	if len(value) != 64 {
		return fmt.Errorf("%s must be 64 lowercase hex characters", label)
	}
	decoded, err := hex.DecodeString(value)
	if err != nil || hex.EncodeToString(decoded) != value {
		return fmt.Errorf("%s must be 64 lowercase hex characters", label)
	}
	return nil
}

func validateEnum(value, label string, allowed ...string) error {
	for _, candidate := range allowed {
		if value == candidate {
			return nil
		}
	}
	return fmt.Errorf("%s has unsupported value %q", label, value)
}
