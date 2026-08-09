package cognitiveatomcontract

import (
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"fmt"
	"regexp"
	"sort"
)

var (
	hashPattern       = regexp.MustCompile(`^[a-f0-9]{64}$`)
	identifierPattern = regexp.MustCompile(`^[a-z0-9][a-z0-9._:/-]*$`)
	projectableStates = map[string]map[string]bool{
		"fact":       stateSet("candidate", "contested"),
		"constraint": stateSet("candidate"),
		"decision":   stateSet("proposed"),
		"inference":  stateSet("candidate"),
		"assumption": stateSet("open", "testing"),
		"hypothesis": stateSet("open", "testing"),
		"unknown":    stateSet("open", "investigating"),
	}
)

func validateAtom(atom *CognitiveAtom) error {
	if atom == nil {
		return fmt.Errorf("atom is nil")
	}
	if atom.APIVersion != APIVersion {
		return fmt.Errorf("api_version must be %q", APIVersion)
	}
	if atom.Kind != Kind {
		return fmt.Errorf("kind must be %q", Kind)
	}
	if atom.Integrity.Canonicalization != Canonicalization {
		return fmt.Errorf("canonicalization must be %q", Canonicalization)
	}
	if err := validateHash("integrity.canonical_sha256", atom.Integrity.CanonicalSHA256); err != nil {
		return err
	}
	if err := validateMetadata(atom.Metadata); err != nil {
		return err
	}
	if err := validateSource(atom.Source); err != nil {
		return err
	}
	if err := validateSpec(atom.Spec); err != nil {
		return err
	}
	expectedID, err := deriveAtomID(atom.Metadata.TaskID, atom.Source.CanonicalSHA256, atom.Metadata.ContextSHA256, atom.Metadata.PolicySHA256, atom.Metadata.SourceTreeSHA256, atom.Metadata.SourceRevision)
	if err != nil {
		return err
	}
	if atom.Metadata.AtomID != expectedID {
		return fmt.Errorf("atom_id mismatch: got %q want %q", atom.Metadata.AtomID, expectedID)
	}
	return nil
}

func validateMetadata(metadata Metadata) error {
	for name, value := range map[string]string{
		"atom_id": metadata.AtomID, "project_id": metadata.ProjectID,
		"scope": metadata.Scope, "source_revision": metadata.SourceRevision,
		"task_id": metadata.TaskID,
	} {
		if err := validateIdentifier(name, value); err != nil {
			return err
		}
	}
	for name, value := range map[string]string{
		"context_sha256": metadata.ContextSHA256, "policy_sha256": metadata.PolicySHA256,
		"source_tree_sha256": metadata.SourceTreeSHA256,
	} {
		if err := validateHash(name, value); err != nil {
			return err
		}
	}
	return nil
}

func validateSource(source Source) error {
	if err := validateHash("source.canonical_sha256", source.CanonicalSHA256); err != nil {
		return err
	}
	if err := validateHash("source.closure_sha256", source.ClosureSHA256); err != nil {
		return err
	}
	for name, value := range map[string]string{
		"source.claim_aggregate_id": source.ClaimAggregateID,
		"source.claim_record_id":    source.ClaimRecordID,
	} {
		if err := validateIdentifier(name, value); err != nil {
			return err
		}
	}
	if source.ClaimSequence < 1 {
		return fmt.Errorf("source.claim_sequence must be positive")
	}
	if source.ClosureRecordCount < 1 || source.ClosureRecordCount > maxArrayItems {
		return fmt.Errorf("source.closure_record_count must be in 1..%d", maxArrayItems)
	}
	if source.ClosureByteCount < 1 || source.ClosureByteCount > maxSetBytes {
		return fmt.Errorf("source.closure_byte_count must be in 1..%d", maxSetBytes)
	}
	if source.RecordKind != "KnowledgeClaim" {
		return fmt.Errorf("source.record_kind must be %q", "KnowledgeClaim")
	}
	return nil
}

func validateSpec(spec Spec) error {
	if err := validateProjectionControls(spec); err != nil {
		return err
	}
	if err := validateProposition(spec.Proposition); err != nil {
		return err
	}
	if err := validateReferenceLists(spec); err != nil {
		return err
	}
	return validateValidity(spec.Validity)
}

func validateProjectionControls(spec Spec) error {
	states, exists := projectableStates[spec.AtomType]
	if !exists {
		return fmt.Errorf("unsupported atom_type %q", spec.AtomType)
	}
	if !states[spec.EpistemicState] {
		return fmt.Errorf("epistemic_state %q is invalid for atom_type %q", spec.EpistemicState, spec.AtomType)
	}
	if spec.AuthorityRef != nil {
		return fmt.Errorf("authority_ref must be null in shadow projection")
	}
	if spec.Hardness != "none" {
		return fmt.Errorf("hardness must be %q in shadow projection", "none")
	}
	if spec.InstructionAllowed {
		return fmt.Errorf("instruction_allowed must be false in shadow projection")
	}
	if spec.ProjectionMode != "shadow" {
		return fmt.Errorf("projection_mode must be %q", "shadow")
	}
	needsConfidence := spec.AtomType == "assumption" || spec.AtomType == "hypothesis" || spec.AtomType == "inference"
	if needsConfidence != (spec.ProjectionConfidenceMicros != nil) {
		return fmt.Errorf("projection_confidence_micros presence does not match atom_type %q", spec.AtomType)
	}
	if spec.ProjectionConfidenceMicros != nil && (*spec.ProjectionConfidenceMicros < 0 || *spec.ProjectionConfidenceMicros > 1000000) {
		return fmt.Errorf("projection_confidence_micros must be in 0..1000000")
	}
	return nil
}

func validateReferenceLists(spec Spec) error {
	lists := map[string][]string{
		"contradicting_evidence_record_ids": spec.ContradictingEvidenceRecordIDs,
		"derived_from_claim_record_ids":     spec.DerivedFromClaimRecordIDs,
		"supporting_evidence_record_ids":    spec.SupportingEvidenceRecordIDs,
	}
	for name, values := range lists {
		if err := validateIdentifierList(name, values); err != nil {
			return err
		}
	}
	supporting := make(map[string]bool, len(spec.SupportingEvidenceRecordIDs))
	for _, recordID := range spec.SupportingEvidenceRecordIDs {
		supporting[recordID] = true
	}
	for _, recordID := range spec.ContradictingEvidenceRecordIDs {
		if supporting[recordID] {
			return fmt.Errorf(
				"supporting_evidence_record_ids and contradicting_evidence_record_ids must be disjoint: %q",
				recordID,
			)
		}
	}
	return nil
}

func validateValidity(validity Validity) error {
	if validity.ValidFromUnixMS < 0 {
		return fmt.Errorf("valid_from_unix_ms must be nonnegative")
	}
	if validity.ValidUntilUnixMS != nil && *validity.ValidUntilUnixMS <= validity.ValidFromUnixMS {
		return fmt.Errorf("valid_until_unix_ms must be greater than valid_from_unix_ms")
	}
	return nil
}

func validateProposition(proposition Proposition) error {
	if err := validateIdentifier("proposition.subject", proposition.Subject); err != nil {
		return err
	}
	if err := validateIdentifier("proposition.predicate", proposition.Predicate); err != nil {
		return err
	}
	if proposition.ObjectType == "artifact_ref" {
		if proposition.ObjectValue.Kind != "string" {
			return fmt.Errorf("artifact_ref object_value must be a string")
		}
		return validateIdentifier("artifact_ref object_value", proposition.ObjectValue.String)
	}
	if proposition.ObjectType != "boolean" && proposition.ObjectType != "integer" && proposition.ObjectType != "null" && proposition.ObjectType != "string" {
		return fmt.Errorf("unsupported object_type %q", proposition.ObjectType)
	}
	if proposition.ObjectValue.Kind != proposition.ObjectType {
		return fmt.Errorf("object_type %q does not match %q object_value", proposition.ObjectType, proposition.ObjectValue.Kind)
	}
	if proposition.ObjectValue.Kind == "string" {
		return validateString(proposition.ObjectValue.String)
	}
	return nil
}

func validateIdentifier(name, value string) error {
	if len(value) > 160 || !identifierPattern.MatchString(value) {
		return fmt.Errorf("%s is not a valid identifier", name)
	}
	return nil
}

func validateHash(name, value string) error {
	if !hashPattern.MatchString(value) {
		return fmt.Errorf("%s must be a lowercase bare SHA-256", name)
	}
	return nil
}

func validateIdentifierList(name string, values []string) error {
	if len(values) > maxArrayItems {
		return fmt.Errorf("%s exceeds %d items", name, maxArrayItems)
	}
	if !sort.StringsAreSorted(values) {
		return fmt.Errorf("%s must already be lexicographically sorted", name)
	}
	for index, value := range values {
		if err := validateIdentifier(name, value); err != nil {
			return err
		}
		if index > 0 && value == values[index-1] {
			return fmt.Errorf("%s contains duplicate %q", name, value)
		}
	}
	return nil
}

func deriveAtomID(taskID, claimDigest, contextDigest, policyDigest, sourceTreeDigest, sourceRevision string) (string, error) {
	if err := validateIdentifier("task_id", taskID); err != nil {
		return "", err
	}
	claim, err := decodeDigest("claim canonical_sha256", claimDigest)
	if err != nil {
		return "", err
	}
	context, err := decodeDigest("context_sha256", contextDigest)
	if err != nil {
		return "", err
	}
	policy, err := decodeDigest("policy_sha256", policyDigest)
	if err != nil {
		return "", err
	}
	sourceTree, err := decodeDigest("source_tree_sha256", sourceTreeDigest)
	if err != nil {
		return "", err
	}
	hasher := sha256.New()
	hasher.Write([]byte(atomIDDomain))
	hasher.Write([]byte{0})
	writeFramedString(hasher.Write, taskID)
	hasher.Write(claim)
	hasher.Write(context)
	hasher.Write(policy)
	hasher.Write(sourceTree)
	writeFramedString(hasher.Write, sourceRevision)
	return "atom-" + hex.EncodeToString(hasher.Sum(nil)), nil
}

func decodeDigest(name, value string) ([]byte, error) {
	if err := validateHash(name, value); err != nil {
		return nil, err
	}
	decoded, err := hex.DecodeString(value)
	if err != nil || len(decoded) != sha256.Size {
		return nil, fmt.Errorf("%s is not a raw SHA-256", name)
	}
	return decoded, nil
}

func writeFramedString(write func([]byte) (int, error), value string) {
	var length [8]byte
	binary.BigEndian.PutUint64(length[:], uint64(len([]byte(value))))
	_, _ = write(length[:])
	_, _ = write([]byte(value))
}

func stateSet(values ...string) map[string]bool {
	result := make(map[string]bool, len(values))
	for _, value := range values {
		result[value] = true
	}
	return result
}
