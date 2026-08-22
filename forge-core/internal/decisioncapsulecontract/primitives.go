package decisioncapsulecontract

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"reflect"
	"sort"

	kd "forgeos/forge-core/internal/kerneldecisioncontract"
	op "forgeos/forge-core/internal/kerneloperationalcontract"
)

func digestValue(value any, domain []byte, maximum int) (string, error) {
	raw, err := canonicalBytes(value, maximum)
	if err != nil {
		return "", err
	}
	digest := sha256.New()
	_, _ = digest.Write(domain)
	_, _ = digest.Write(raw)
	return hex.EncodeToString(digest.Sum(nil)), nil
}

func validateIdentity(id, digest, prefix, label string, allowBlank bool) error {
	if allowBlank && id == "" && digest == "" {
		return nil
	}
	return validateReference(id, digest, prefix, label)
}

func validateReference(id, digest, prefix, label string) error {
	if len(digest) != 64 {
		return fmt.Errorf("%s digest must be 64 lowercase hex characters", label)
	}
	for _, character := range []byte(digest) {
		if (character < '0' || character > '9') && (character < 'a' || character > 'f') {
			return fmt.Errorf("%s digest must be 64 lowercase hex characters", label)
		}
	}
	if id != prefix+digest {
		return fmt.Errorf("%s identity must bind its digest", label)
	}
	return nil
}

func validateReplayAttestations(value ReplayAttestations) error {
	if value != (ReplayAttestations{}) {
		return fmt.Errorf("all thirty-two replay attestations must be false")
	}
	return nil
}

func validateArtifactRef(value *op.ArtifactRef) error {
	if value == nil {
		return fmt.Errorf("ArtifactRef is nil")
	}
	raw, err := op.CanonicalJSON(value)
	if err != nil {
		return err
	}
	_, err = op.DecodeArtifactRef(raw)
	return err
}

func validateDecisionClosure(value *kd.KernelDecisionReferenceClosure) (
	*kd.KernelDecisionReferenceClosure, error,
) {
	if value == nil {
		return nil, fmt.Errorf("ADR-0090 decision closure is nil")
	}
	if err := kd.ValidateClosure(value); err != nil {
		return nil, fmt.Errorf("decision_closure: %w", err)
	}
	original, err := canonicalDecisionClosure(value)
	if err != nil {
		return nil, fmt.Errorf("decision_closure canonical bytes: %w", err)
	}
	blank := *value
	blank.ClosureID, blank.ClosureSHA256 = "", ""
	preimage, err := canonicalDecisionClosure(&blank)
	if err != nil {
		return nil, fmt.Errorf("decision_closure blank preimage: %w", err)
	}
	hasher := sha256.New()
	_, _ = hasher.Write(decisionClosureDomain)
	_, _ = hasher.Write(preimage)
	digest := hex.EncodeToString(hasher.Sum(nil))
	resealed := blank
	resealed.ClosureID, resealed.ClosureSHA256 = decisionClosurePrefix+digest, digest
	if !reflect.DeepEqual(value, &resealed) {
		return nil, fmt.Errorf("embedded ADR-0090 closure differs after exact reseal")
	}
	resealedRaw, err := canonicalDecisionClosure(&resealed)
	if err != nil {
		return nil, fmt.Errorf("resealed decision_closure canonical bytes: %w", err)
	}
	if !bytes.Equal(original, resealedRaw) {
		return nil, fmt.Errorf("embedded ADR-0090 closure differs after exact reseal")
	}
	return cloneDecisionClosure(resealedRaw)
}

func canonicalDecisionClosure(value any) ([]byte, error) {
	raw, err := kd.CanonicalJSON(value)
	if err != nil {
		return nil, err
	}
	if len(raw) == 0 || len(raw) > maxDecisionClosureBytes {
		return nil, fmt.Errorf("ADR-0090 canonical bytes must be 1..%d", maxDecisionClosureBytes)
	}
	return raw, nil
}

func cloneDecisionClosure(raw []byte) (*kd.KernelDecisionReferenceClosure, error) {
	var cloned kd.KernelDecisionReferenceClosure
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&cloned); err != nil {
		return nil, fmt.Errorf("decision_closure clone decode: %w", err)
	}
	if err := requireEOF(decoder); err != nil {
		return nil, err
	}
	return &cloned, nil
}

func validateUniqueKeys(keys []string, label string, sortedOrder bool) error {
	seen := make(map[string]struct{}, len(keys))
	for _, key := range keys {
		if _, exists := seen[key]; exists {
			return fmt.Errorf("%s must be unique", label)
		}
		seen[key] = struct{}{}
	}
	if sortedOrder && !sort.StringsAreSorted(keys) {
		return fmt.Errorf("%s must preserve canonical identity order", label)
	}
	return nil
}
