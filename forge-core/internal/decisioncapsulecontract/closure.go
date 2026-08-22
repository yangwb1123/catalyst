package decisioncapsulecontract

import (
	"fmt"

	op "forgeos/forge-core/internal/kerneloperationalcontract"
)

func validateReflectionRefs(values []op.ArtifactRef) error {
	if values == nil || len(values) > maxReflectionReportRefs {
		return fmt.Errorf("reflection_report_artifact_refs cardinality must be 0..%d",
			maxReflectionReportRefs)
	}
	keys := make([]string, len(values))
	for index := range values {
		if err := validateArtifactRef(&values[index]); err != nil {
			return fmt.Errorf("reflection_report_artifact_refs[%d]: %w", index, err)
		}
		if values[index].ArtifactKind != "reflection_report" {
			return fmt.Errorf("reflection report ArtifactRefs require reflection_report kind")
		}
		raw, _ := op.CanonicalJSON(&values[index])
		keys[index] = string(raw)
	}
	return validateUniqueKeys(keys, "reflection_report_artifact_refs", true)
}

func validateClosureLocal(value *StructuralReplayClosure, allowBlank bool,
	ignoreIdentity bool) error {
	if value == nil {
		return fmt.Errorf("StructuralReplayClosure is nil")
	}
	if value.APIVersion != closureAPI || value.Canonicalization != canonicalization ||
		value.Kind != closureKind || value.Result != successMarker {
		return fmt.Errorf("StructuralReplayClosure constants or result marker differ")
	}
	if !ignoreIdentity {
		if err := validateIdentity(value.ClosureID, value.ClosureSHA256,
			closurePrefix, "outer closure", allowBlank); err != nil {
			return err
		}
	}
	if err := validateReplayAttestations(value.Attestations); err != nil {
		return err
	}
	if err := validateReflectionRefs(value.ReflectionReportArtifactRefs); err != nil {
		return err
	}
	if err := validateBranchLocal(&value.EvaluationBranch, false); err != nil {
		return fmt.Errorf("evaluation_branch: %w", err)
	}
	if err := validateCapsuleLocal(&value.DecisionCapsule, false, false); err != nil {
		return fmt.Errorf("decision_capsule: %w", err)
	}
	return nil
}

func validateClosureShape(value *StructuralReplayClosure, allowBlank bool) error {
	if err := validateClosureLocal(value, allowBlank, false); err != nil {
		return err
	}
	if err := requireBranchComparison(
		&value.EvaluationBranch, &value.DecisionCapsule); err != nil {
		return fmt.Errorf("evaluation_branch: %w", err)
	}
	if _, err := measureTypedJSON(value, maxClosureBytes); err != nil {
		return err
	}
	if _, err := validateCapsulePrepared(&value.DecisionCapsule, false); err != nil {
		return fmt.Errorf("decision_capsule: %w", err)
	}
	if err := validateBranchPrepared(&value.EvaluationBranch,
		&value.DecisionCapsule, false); err != nil {
		return fmt.Errorf("evaluation_branch: %w", err)
	}
	return nil
}

func deriveClosurePrepared(capsule *DecisionCapsule,
	reflectionReportArtifactRefs []op.ArtifactRef) (*StructuralReplayClosure, error) {
	branch, err := deriveBranchPrepared(capsule)
	if err != nil {
		return nil, err
	}
	closure := &StructuralReplayClosure{
		APIVersion:                   closureAPI,
		Canonicalization:             canonicalization,
		DecisionCapsule:              *capsule,
		EvaluationBranch:             *branch,
		Kind:                         closureKind,
		ReflectionReportArtifactRefs: append([]op.ArtifactRef{}, reflectionReportArtifactRefs...),
		Result:                       successMarker,
	}
	if err := validateReflectionRefs(closure.ReflectionReportArtifactRefs); err != nil {
		return nil, err
	}
	digest, err := digestValue(closure, closureDomain, maxClosureBytes)
	if err != nil {
		return nil, fmt.Errorf("outer closure blank preimage: %w", err)
	}
	closure.ClosureID, closure.ClosureSHA256 = closurePrefix+digest, digest
	if _, err := canonicalBytes(closure, maxClosureBytes); err != nil {
		return nil, fmt.Errorf("sealed outer closure: %w", err)
	}
	return closure, nil
}

func StructuralReplayClosureDigest(value *StructuralReplayClosure) (string, error) {
	if value == nil {
		return "", fmt.Errorf("StructuralReplayClosure is nil")
	}
	if err := validateClosureLocal(value, false, true); err != nil {
		return "", err
	}
	if _, err := measureTypedJSON(value, maxClosureBytes); err != nil {
		return "", err
	}
	blank := *value
	blank.ClosureID, blank.ClosureSHA256 = "", ""
	if err := validateClosureShape(&blank, true); err != nil {
		return "", err
	}
	return digestValue(&blank, closureDomain, maxClosureBytes)
}

func ValidateStructuralReplayClosure(value *StructuralReplayClosure) error {
	if err := validateClosureShape(value, false); err != nil {
		return err
	}
	blank := *value
	blank.ClosureID, blank.ClosureSHA256 = "", ""
	digest, err := digestValue(&blank, closureDomain, maxClosureBytes)
	if err != nil {
		return fmt.Errorf("outer closure blank preimage: %w", err)
	}
	if value.ClosureSHA256 != digest {
		return fmt.Errorf("closure_sha256 does not match canonical preimage")
	}
	_, err = canonicalBytes(value, maxClosureBytes)
	return err
}

func SealStructuralReplayClosure(value *StructuralReplayClosure) (
	*StructuralReplayClosure, error,
) {
	if value == nil || value.ClosureID != "" || value.ClosureSHA256 != "" {
		return nil, fmt.Errorf("sealing outer closure requires blank own identity")
	}
	if err := validateClosureLocal(value, true, false); err != nil {
		return nil, err
	}
	if err := validateClosureShape(value, true); err != nil {
		return nil, err
	}
	sealed, err := cloneValue(value, maxClosureBytes)
	if err != nil {
		return nil, err
	}
	digest, err := digestValue(sealed, closureDomain, maxClosureBytes)
	if err != nil {
		return nil, fmt.Errorf("outer closure blank preimage: %w", err)
	}
	sealed.ClosureID, sealed.ClosureSHA256 = closurePrefix+digest, digest
	if _, err := canonicalBytes(sealed, maxClosureBytes); err != nil {
		return nil, fmt.Errorf("sealed outer closure: %w", err)
	}
	return sealed, nil
}

func DeriveStructuralReplayClosure(capsule *DecisionCapsule,
	reflectionReportArtifactRefs []op.ArtifactRef) (*StructuralReplayClosure, error) {
	if err := validateReflectionRefs(reflectionReportArtifactRefs); err != nil {
		return nil, err
	}
	if err := validateCapsuleLocal(capsule, false, false); err != nil {
		return nil, err
	}
	if _, err := validateCapsulePrepared(capsule, false); err != nil {
		return nil, err
	}
	cloned, err := cloneValue(capsule, maxCapsuleBytes)
	if err != nil {
		return nil, err
	}
	return deriveClosurePrepared(cloned, reflectionReportArtifactRefs)
}

func DecodeStructuralReplayClosure(raw []byte) (*StructuralReplayClosure, error) {
	value, err := decodeExact[StructuralReplayClosure](raw, maxClosureBytes)
	if err != nil {
		return nil, err
	}
	return value, ValidateStructuralReplayClosure(value)
}
