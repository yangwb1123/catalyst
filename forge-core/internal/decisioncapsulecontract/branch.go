package decisioncapsulecontract

import (
	"fmt"
	"reflect"
)

func deriveBranchPrepared(capsule *DecisionCapsule) (*EvaluationBranch, error) {
	branch := &EvaluationBranch{
		APIVersion:       branchAPI,
		BranchMode:       branchMode,
		Canonicalization: canonicalization,
		CapsuleRef: CapsuleRef{
			CapsuleID: capsule.CapsuleID, CapsuleSHA256: capsule.CapsuleSHA256,
		},
		ComparisonResult: comparisonResult,
		DecisionClosureRef: ClosureRef{
			ClosureID:     capsule.DecisionClosure.ClosureID,
			ClosureSHA256: capsule.DecisionClosure.ClosureSHA256,
		},
		Kind: branchKind,
		ManifestRef: ManifestRef{
			ManifestID:     capsule.ReplayManifest.ManifestID,
			ManifestSHA256: capsule.ReplayManifest.ManifestSHA256,
		},
	}
	digest, err := digestValue(branch, branchDomain, maxBranchBytes)
	if err != nil {
		return nil, fmt.Errorf("branch blank preimage: %w", err)
	}
	branch.BranchID, branch.BranchSHA256 = branchPrefix+digest, digest
	if _, err := canonicalBytes(branch, maxBranchBytes); err != nil {
		return nil, fmt.Errorf("sealed branch: %w", err)
	}
	return branch, nil
}

func validateBranchLocal(value *EvaluationBranch, allowBlank bool) error {
	if value == nil {
		return fmt.Errorf("EvaluationBranch is nil")
	}
	if value.APIVersion != branchAPI || value.BranchMode != branchMode ||
		value.Canonicalization != canonicalization ||
		value.ComparisonResult != comparisonResult || value.Kind != branchKind {
		return fmt.Errorf("EvaluationBranch constants or comparison result differ")
	}
	if value.EffectReplayAllowed || value.HistoryRewriteAllowed {
		return fmt.Errorf("branch effect replay and history rewrite controls must be false")
	}
	if err := validateIdentity(value.BranchID, value.BranchSHA256,
		branchPrefix, "branch", allowBlank); err != nil {
		return err
	}
	if err := validateReplayAttestations(value.Attestations); err != nil {
		return err
	}
	if err := validateReference(value.CapsuleRef.CapsuleID, value.CapsuleRef.CapsuleSHA256,
		capsulePrefix, "capsule_ref"); err != nil {
		return err
	}
	if err := validateReference(value.DecisionClosureRef.ClosureID,
		value.DecisionClosureRef.ClosureSHA256, decisionClosurePrefix,
		"decision_closure_ref"); err != nil {
		return err
	}
	if err := validateReference(value.ManifestRef.ManifestID, value.ManifestRef.ManifestSHA256,
		manifestPrefix, "manifest_ref"); err != nil {
		return err
	}
	return nil
}

func validateBranchShape(value *EvaluationBranch, allowBlank bool) error {
	if err := validateBranchLocal(value, allowBlank); err != nil {
		return err
	}
	_, err := measureTypedJSON(value, maxBranchBytes)
	return err
}

func requireBranchComparison(value *EvaluationBranch, capsule *DecisionCapsule) error {
	if capsule == nil || value.CapsuleRef != (CapsuleRef{
		CapsuleID: capsule.CapsuleID, CapsuleSHA256: capsule.CapsuleSHA256}) ||
		value.DecisionClosureRef != (ClosureRef{
			ClosureID:     capsule.DecisionClosure.ClosureID,
			ClosureSHA256: capsule.DecisionClosure.ClosureSHA256}) ||
		value.ManifestRef != (ManifestRef{
			ManifestID:     capsule.ReplayManifest.ManifestID,
			ManifestSHA256: capsule.ReplayManifest.ManifestSHA256}) {
		return fmt.Errorf("branch must be the unique structural comparison for its capsule")
	}
	return nil
}

func validateBranchPrepared(value *EvaluationBranch, capsule *DecisionCapsule,
	allowBlank bool) error {
	if err := validateBranchShape(value, allowBlank); err != nil {
		return err
	}
	blank := *value
	blank.BranchID, blank.BranchSHA256 = "", ""
	expected, err := deriveBranchPrepared(capsule)
	if err != nil {
		return err
	}
	expected.BranchID, expected.BranchSHA256 = "", ""
	if !reflect.DeepEqual(&blank, expected) {
		return fmt.Errorf("branch must be the unique structural comparison for its capsule")
	}
	digest, err := digestValue(&blank, branchDomain, maxBranchBytes)
	if err != nil {
		return fmt.Errorf("branch blank preimage: %w", err)
	}
	if !allowBlank && value.BranchSHA256 != digest {
		return fmt.Errorf("branch_sha256 does not match canonical preimage")
	}
	if _, err := canonicalBytes(value, maxBranchBytes); err != nil {
		return fmt.Errorf("sealed branch: %w", err)
	}
	return nil
}

func EvaluationBranchDigest(value *EvaluationBranch, capsule *DecisionCapsule) (string, error) {
	if value == nil {
		return "", fmt.Errorf("EvaluationBranch is nil")
	}
	blank := *value
	blank.BranchID, blank.BranchSHA256 = "", ""
	if err := validateBranchLocal(&blank, true); err != nil {
		return "", err
	}
	if err := validateCapsuleLocal(capsule, false, false); err != nil {
		return "", err
	}
	if err := requireBranchComparison(&blank, capsule); err != nil {
		return "", err
	}
	if _, err := measureTypedJSON(value, maxBranchBytes); err != nil {
		return "", err
	}
	if err := validateBranchShape(&blank, true); err != nil {
		return "", err
	}
	if _, err := validateCapsulePrepared(capsule, false); err != nil {
		return "", err
	}
	if err := validateBranchPrepared(&blank, capsule, true); err != nil {
		return "", err
	}
	return digestValue(&blank, branchDomain, maxBranchBytes)
}

func ValidateEvaluationBranch(value *EvaluationBranch, capsule *DecisionCapsule) error {
	if err := validateBranchLocal(value, false); err != nil {
		return err
	}
	if err := validateCapsuleLocal(capsule, false, false); err != nil {
		return err
	}
	if err := requireBranchComparison(value, capsule); err != nil {
		return err
	}
	if _, err := measureTypedJSON(value, maxBranchBytes); err != nil {
		return err
	}
	if _, err := validateCapsulePrepared(capsule, false); err != nil {
		return err
	}
	return validateBranchPrepared(value, capsule, false)
}

func SealEvaluationBranch(value *EvaluationBranch,
	capsule *DecisionCapsule) (*EvaluationBranch, error) {
	if value == nil || value.BranchID != "" || value.BranchSHA256 != "" {
		return nil, fmt.Errorf("sealing branch requires blank own identity")
	}
	if err := validateBranchLocal(value, true); err != nil {
		return nil, err
	}
	if err := validateCapsuleLocal(capsule, false, false); err != nil {
		return nil, err
	}
	if err := requireBranchComparison(value, capsule); err != nil {
		return nil, err
	}
	if _, err := measureTypedJSON(value, maxBranchBytes); err != nil {
		return nil, err
	}
	if _, err := validateCapsulePrepared(capsule, false); err != nil {
		return nil, err
	}
	if err := validateBranchPrepared(value, capsule, true); err != nil {
		return nil, err
	}
	return deriveBranchPrepared(capsule)
}

func DeriveEvaluationBranch(capsule *DecisionCapsule) (*EvaluationBranch, error) {
	if err := validateCapsuleLocal(capsule, false, false); err != nil {
		return nil, err
	}
	if _, err := validateCapsulePrepared(capsule, false); err != nil {
		return nil, err
	}
	return deriveBranchPrepared(capsule)
}

func DecodeEvaluationBranch(raw []byte, capsule *DecisionCapsule) (*EvaluationBranch, error) {
	value, err := decodeExact[EvaluationBranch](raw, maxBranchBytes)
	if err != nil {
		return nil, err
	}
	return value, ValidateEvaluationBranch(value, capsule)
}
