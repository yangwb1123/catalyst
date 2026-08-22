package outputbinding

import "fmt"

// SealPreflight sets fixed fields and seals the challenge-bearing preflight.
func SealPreflight(binding PreflightBinding) (PreflightBinding, error) {
	binding.APIVersion = preflightAPI
	binding.Canonicalization = canonicalization
	binding.ProfileID = localProfile
	binding.BindingSHA256 = ""
	if err := validatePreflightPayload(binding); err != nil {
		return PreflightBinding{}, err
	}
	digest, err := preflightDigest(binding)
	if err != nil {
		return PreflightBinding{}, err
	}
	binding.BindingSHA256 = digest
	return binding, nil
}

// ValidatePreflight verifies the complete pre-spawn local digest binding.
func ValidatePreflight(binding PreflightBinding) error {
	if err := validatePreflightPayload(binding); err != nil {
		return err
	}
	if err := requireDigest("preflight binding_sha256", binding.BindingSHA256); err != nil {
		return err
	}
	digest, err := preflightDigest(binding)
	if err != nil {
		return err
	}
	if digest != binding.BindingSHA256 {
		return fmt.Errorf("output binding: preflight binding_sha256 mismatch")
	}
	return nil
}

func validatePreflightPayload(binding PreflightBinding) error {
	if binding.APIVersion != preflightAPI || binding.Canonicalization != canonicalization ||
		binding.ProfileID != localProfile {
		return fmt.Errorf("output binding: preflight fixed fields drifted")
	}
	if binding.Attempt < 1 || binding.Attempt > maxSequence {
		return fmt.Errorf("output binding: preflight attempt must be in 1..%d", maxSequence)
	}
	for label, value := range map[string]string{
		"phase": binding.Phase, "run_id": binding.RunID, "workflow": binding.Workflow,
	} {
		if err := validateIdentifier(label, value); err != nil {
			return fmt.Errorf("output binding: preflight: %w", err)
		}
	}
	return validatePreflightDigests(binding)
}

func validatePreflightDigests(binding PreflightBinding) error {
	fields := map[string]string{
		"artifact_inputs_sha256":      binding.ArtifactInputsSHA256,
		"challenge":                   binding.Challenge,
		"local_runtime_policy_sha256": binding.LocalRuntimePolicySHA256,
		"prompt_context_sha256":       binding.PromptContextSHA256,
		"source_before_sha256":        binding.SourceBeforeSHA256,
		"workflow_sha256":             binding.WorkflowSHA256,
	}
	for label, value := range fields {
		if err := requireDigest("preflight "+label, value); err != nil {
			return err
		}
	}
	return nil
}

func preflightDigest(binding PreflightBinding) (string, error) {
	binding.BindingSHA256 = ""
	encoded, err := canonicalJSON(binding, maxPreflightBytes)
	if err != nil {
		return "", err
	}
	return domainDigest(preflightDomain, encoded), nil
}

// CanonicalPreflightJSON returns exact compact canonical bytes without an LF.
func CanonicalPreflightJSON(binding PreflightBinding) ([]byte, error) {
	if err := ValidatePreflight(binding); err != nil {
		return nil, err
	}
	return canonicalJSON(binding, maxPreflightBytes)
}

// DecodeCanonicalPreflight accepts only the exact v1 canonical wire form.
func DecodeCanonicalPreflight(data []byte) (PreflightBinding, error) {
	var binding PreflightBinding
	err := decodeExact(data, maxPreflightBytes, &binding,
		func() error { return ValidatePreflight(binding) },
		func() ([]byte, error) { return canonicalJSON(binding, maxPreflightBytes) })
	return binding, err
}
