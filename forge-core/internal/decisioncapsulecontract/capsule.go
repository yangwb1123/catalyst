package decisioncapsulecontract

import (
	"fmt"

	kd "forgeos/forge-core/internal/kerneldecisioncontract"
)

func validateCapsuleLocal(value *DecisionCapsule, allowBlank bool, ignoreIdentity bool) error {
	if value == nil {
		return fmt.Errorf("DecisionCapsule is nil")
	}
	if value.APIVersion != capsuleAPI || value.Canonicalization != canonicalization ||
		value.CapsuleMode != capsuleMode || value.Kind != capsuleKind ||
		value.Result != capsuleResult {
		return fmt.Errorf("DecisionCapsule constants or result marker differ")
	}
	if !ignoreIdentity {
		if err := validateIdentity(value.CapsuleID, value.CapsuleSHA256,
			capsulePrefix, "capsule", allowBlank); err != nil {
			return err
		}
	}
	if err := validateReplayAttestations(value.Attestations); err != nil {
		return err
	}
	return validateManifestLocal(&value.ReplayManifest, false)
}

func validateCapsulePrepared(value *DecisionCapsule, allowBlank bool) (
	*kd.KernelDecisionReferenceClosure, error,
) {
	if err := validateCapsuleLocal(value, allowBlank, false); err != nil {
		return nil, err
	}
	if _, err := measureTypedJSON(value, maxCapsuleBytes); err != nil {
		return nil, err
	}
	if err := requireManifestProjection(&value.ReplayManifest, &value.DecisionClosure); err != nil {
		return nil, fmt.Errorf("replay_manifest: %w", err)
	}
	closure, err := validateDecisionClosure(&value.DecisionClosure)
	if err != nil {
		return nil, err
	}
	if err := validateManifestPrepared(&value.ReplayManifest, closure, false); err != nil {
		return nil, fmt.Errorf("replay_manifest: %w", err)
	}
	blank := *value
	blank.CapsuleID, blank.CapsuleSHA256 = "", ""
	digest, err := digestValue(&blank, capsuleDomain, maxCapsuleBytes)
	if err != nil {
		return nil, fmt.Errorf("capsule blank preimage: %w", err)
	}
	if !allowBlank && value.CapsuleSHA256 != digest {
		return nil, fmt.Errorf("capsule_sha256 does not match canonical preimage")
	}
	if _, err := canonicalBytes(value, maxCapsuleBytes); err != nil {
		return nil, fmt.Errorf("sealed capsule: %w", err)
	}
	return closure, nil
}

func deriveCapsulePrepared(closure *kd.KernelDecisionReferenceClosure) (*DecisionCapsule, error) {
	manifest, err := deriveManifestPrepared(closure)
	if err != nil {
		return nil, err
	}
	capsule := &DecisionCapsule{
		APIVersion:       capsuleAPI,
		Canonicalization: canonicalization,
		CapsuleMode:      capsuleMode,
		DecisionClosure:  *closure,
		Kind:             capsuleKind,
		ReplayManifest:   *manifest,
		Result:           capsuleResult,
	}
	digest, err := digestValue(capsule, capsuleDomain, maxCapsuleBytes)
	if err != nil {
		return nil, fmt.Errorf("capsule blank preimage: %w", err)
	}
	capsule.CapsuleID, capsule.CapsuleSHA256 = capsulePrefix+digest, digest
	if _, err := canonicalBytes(capsule, maxCapsuleBytes); err != nil {
		return nil, fmt.Errorf("sealed capsule: %w", err)
	}
	return capsule, nil
}

func DecisionCapsuleDigest(value *DecisionCapsule) (string, error) {
	if value == nil {
		return "", fmt.Errorf("DecisionCapsule is nil")
	}
	if err := validateCapsuleLocal(value, false, true); err != nil {
		return "", err
	}
	if _, err := measureTypedJSON(value, maxCapsuleBytes); err != nil {
		return "", err
	}
	blank := *value
	blank.CapsuleID, blank.CapsuleSHA256 = "", ""
	if _, err := validateCapsulePrepared(&blank, true); err != nil {
		return "", err
	}
	return digestValue(&blank, capsuleDomain, maxCapsuleBytes)
}

func ValidateDecisionCapsule(value *DecisionCapsule) error {
	_, err := validateCapsulePrepared(value, false)
	return err
}

func SealDecisionCapsule(value *DecisionCapsule) (*DecisionCapsule, error) {
	if value == nil || value.CapsuleID != "" || value.CapsuleSHA256 != "" {
		return nil, fmt.Errorf("sealing capsule requires blank own identity")
	}
	closure, err := validateCapsulePrepared(value, true)
	if err != nil {
		return nil, err
	}
	clonedClosure, err := cloneValue(closure, maxCapsuleBytes)
	if err != nil {
		return nil, err
	}
	return deriveCapsulePrepared(clonedClosure)
}

func DeriveDecisionCapsule(decisionClosure *kd.KernelDecisionReferenceClosure) (
	*DecisionCapsule, error,
) {
	closure, err := validateDecisionClosure(decisionClosure)
	if err != nil {
		return nil, err
	}
	return deriveCapsulePrepared(closure)
}

func DecodeDecisionCapsule(raw []byte) (*DecisionCapsule, error) {
	value, err := decodeExact[DecisionCapsule](raw, maxCapsuleBytes)
	if err != nil {
		return nil, err
	}
	return value, ValidateDecisionCapsule(value)
}
