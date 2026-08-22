package outputbinding

import (
	"fmt"
	"sort"
)

// SealRuntimePolicy returns a detached, sorted, self-digested local policy
// observation. It does not decide whether that policy authorizes execution.
func SealRuntimePolicy(policy RuntimePolicyBinding) (RuntimePolicyBinding, error) {
	policy.APIVersion = policyAPI
	policy.Canonicalization = canonicalization
	policy.BindingSHA256 = ""
	policy.Gates = append([]string(nil), policy.Gates...)
	if policy.Gates == nil {
		policy.Gates = make([]string, 0)
	}
	sort.Strings(policy.Gates)
	if err := validateRuntimePolicyPayload(policy); err != nil {
		return RuntimePolicyBinding{}, err
	}
	digest, err := runtimePolicyDigest(policy)
	if err != nil {
		return RuntimePolicyBinding{}, err
	}
	policy.BindingSHA256 = digest
	return policy, nil
}

// ValidateRuntimePolicy verifies exact local inputs and the policy self-digest.
func ValidateRuntimePolicy(policy RuntimePolicyBinding) error {
	if err := validateRuntimePolicyPayload(policy); err != nil {
		return err
	}
	if err := requireDigest("runtime policy binding_sha256", policy.BindingSHA256); err != nil {
		return err
	}
	digest, err := runtimePolicyDigest(policy)
	if err != nil {
		return err
	}
	if digest != policy.BindingSHA256 {
		return fmt.Errorf("output binding: runtime policy binding_sha256 mismatch")
	}
	return nil
}

func validateRuntimePolicyPayload(policy RuntimePolicyBinding) error {
	if policy.APIVersion != policyAPI || policy.Canonicalization != canonicalization ||
		policy.OutputBindingContract != localProfile {
		return fmt.Errorf("output binding: runtime policy fixed fields drifted")
	}
	if err := validateRuntimePolicyText(policy); err != nil {
		return err
	}
	if !oneOf(policy.Materiality, "L0", "L1", "L2", "L3", "L4", "materiality_not_bound") {
		return fmt.Errorf("output binding: runtime policy materiality is invalid")
	}
	if policy.Gates == nil || len(policy.Gates) > 64 {
		return fmt.Errorf("output binding: runtime policy gates must be a non-null bounded array")
	}
	return validateGates(policy.Gates)
}

func validateRuntimePolicyText(policy RuntimePolicyBinding) error {
	fields := map[string]string{
		"agent": policy.Agent, "design_depth": policy.DesignDepth,
		"discover_depth":   policy.DiscoverDepth,
		"evolve_authority": policy.EvolveAuthority, "evolve_depth": policy.EvolveDepth,
		"lifecycle": policy.Lifecycle, "mode": policy.Mode, "model": policy.Model,
		"phase": policy.Phase, "review_depth": policy.ReviewDepth, "stage": policy.Stage,
	}
	for label, value := range fields {
		if err := validateIdentifier(label, value); err != nil {
			return fmt.Errorf("output binding: runtime policy: %w", err)
		}
	}
	for label, value := range map[string]string{"effect": policy.Effect, "verdict_contract": policy.VerdictContract} {
		if err := validateOptionalIdentifier(label, value); err != nil {
			return fmt.Errorf("output binding: runtime policy %s: %w", label, err)
		}
	}
	if err := validateWireText(policy.Executor, false, maxReferenceBytes); err != nil {
		return fmt.Errorf("output binding: runtime policy executor: %w", err)
	}
	return requireDigest("runtime policy workflow_sha256", policy.WorkflowSHA256)
}

func validateGates(gates []string) error {
	var prior string
	for index, gate := range gates {
		if err := validateIdentifier("gate", gate); err != nil {
			return fmt.Errorf("output binding: runtime policy gate %d: %w", index, err)
		}
		if index > 0 && prior >= gate {
			return fmt.Errorf("output binding: runtime policy gates must be sorted and unique")
		}
		prior = gate
	}
	return nil
}

func runtimePolicyDigest(policy RuntimePolicyBinding) (string, error) {
	policy.BindingSHA256 = ""
	encoded, err := canonicalJSON(policy, maxPolicyBytes)
	if err != nil {
		return "", err
	}
	return domainDigest(policyDomain, encoded), nil
}

// CanonicalRuntimePolicyJSON returns exact compact canonical bytes without LF.
func CanonicalRuntimePolicyJSON(policy RuntimePolicyBinding) ([]byte, error) {
	if err := ValidateRuntimePolicy(policy); err != nil {
		return nil, err
	}
	return canonicalJSON(policy, maxPolicyBytes)
}

// DecodeCanonicalRuntimePolicy accepts only the exact v1 canonical wire form.
func DecodeCanonicalRuntimePolicy(data []byte) (RuntimePolicyBinding, error) {
	var policy RuntimePolicyBinding
	err := decodeExact(data, maxPolicyBytes, &policy,
		func() error { return ValidateRuntimePolicy(policy) },
		func() ([]byte, error) { return canonicalJSON(policy, maxPolicyBytes) })
	return policy, err
}
