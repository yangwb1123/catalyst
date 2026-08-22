package kerneldecisioncontract

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"

	op "forgeos/forge-core/internal/kerneloperationalcontract"
)

func validateClosureBody(value *KernelDecisionReferenceClosure, blank bool) error {
	if value == nil {
		return fmt.Errorf("KernelDecisionReferenceClosure is nil")
	}
	if value.APIVersion != closureAPI || value.Canonicalization != canonicalization ||
		value.Kind != closureKind || value.Result != SuccessMarker {
		return fmt.Errorf("KernelDecisionReferenceClosure constants or marker differ")
	}
	if err := identity(value.ClosureID, value.ClosureSHA256, closurePrefix, "closure", blank); err != nil {
		return err
	}
	if err := validateAttestations(value.Attestations); err != nil {
		return err
	}
	if err := sortAtoms(value.CognitiveAtoms); err != nil {
		return err
	}
	if _, err := canonicalBytes(value.CognitiveAtoms, maxAtomSetBytes); err != nil {
		return fmt.Errorf("cognitive_atoms: %w", err)
	}
	if err := ValidateDecisionTransaction(&value.DecisionTransaction); err != nil {
		return err
	}
	if err := op.ValidateClosure(&value.OperationalClosure); err != nil {
		return fmt.Errorf("operational_closure: %w", err)
	}
	return validateReferenceGraph(value.CognitiveAtoms, &value.DecisionTransaction,
		&value.OperationalClosure)
}

func closureDigest(value *KernelDecisionReferenceClosure) (string, error) {
	blank := *value
	blank.ClosureID, blank.ClosureSHA256 = "", ""
	if err := validateClosureBody(&blank, true); err != nil {
		return "", err
	}
	raw, err := canonicalBytes(&blank, maxClosureBytes)
	if err != nil {
		return "", err
	}
	digest := sha256.New()
	digest.Write(closureDomain)
	digest.Write(raw)
	return hex.EncodeToString(digest.Sum(nil)), nil
}

func ValidateClosure(value *KernelDecisionReferenceClosure) error {
	if err := validateClosureBody(value, false); err != nil {
		return err
	}
	digest, err := closureDigest(value)
	if err != nil {
		return err
	}
	if value.ClosureSHA256 != digest {
		return fmt.Errorf("closure_sha256 does not match canonical preimage")
	}
	_, err = canonicalBytes(value, maxClosureBytes)
	return err
}

func SealClosure(value *KernelDecisionReferenceClosure) (*KernelDecisionReferenceClosure, error) {
	if value == nil || value.ClosureID != "" || value.ClosureSHA256 != "" {
		return nil, fmt.Errorf("sealing closure requires blank identity")
	}
	sealed, err := cloneValue(value)
	if err != nil {
		return nil, err
	}
	digest, err := closureDigest(sealed)
	if err != nil {
		return nil, err
	}
	sealed.ClosureID, sealed.ClosureSHA256 = closurePrefix+digest, digest
	return sealed, ValidateClosure(sealed)
}

func DecodeClosure(raw []byte) (*KernelDecisionReferenceClosure, error) {
	var value KernelDecisionReferenceClosure
	if err := decodeTyped(raw, maxClosureBytes, &value); err != nil {
		return nil, err
	}
	if _, err := op.DecodeClosure(mustOperationalBytes(&value.OperationalClosure)); err != nil {
		return nil, fmt.Errorf("operational_closure: %w", err)
	}
	return &value, ValidateClosure(&value)
}

func mustOperationalBytes(value *op.KernelOperationalReferenceClosure) []byte {
	raw, err := op.CanonicalJSON(value)
	if err != nil {
		return nil
	}
	return raw
}

func CanonicalJSON(value any) ([]byte, error) {
	return canonicalBytes(value, maxClosureBytes)
}
