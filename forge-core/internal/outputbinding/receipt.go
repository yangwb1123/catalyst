package outputbinding

import "fmt"

// SealReceipt sets fixed fields, detaches slices and pointers, and computes the
// receipt self-digest. Nested manifests and policy must already be sealed.
func SealReceipt(receipt AgentOutputReceipt) (AgentOutputReceipt, error) {
	receipt = cloneReceipt(receipt)
	receipt.APIVersion = receiptAPI
	receipt.Canonicalization = canonicalization
	receipt.Kind = receiptKind
	receipt.ProfileID = localProfile
	receipt.SourceStateProfile = sourceStateProfile
	receipt.ReceiptSHA256 = ""
	if err := validateReceiptPayload(receipt); err != nil {
		return AgentOutputReceipt{}, err
	}
	digest, err := receiptDigest(receipt)
	if err != nil {
		return AgentOutputReceipt{}, err
	}
	receipt.ReceiptSHA256 = digest
	return receipt, nil
}

// ValidateReceipt verifies all nested digests, cross-bindings, preflight
// reconstruction, chain-link shape, bounds, and the receipt self-digest.
func ValidateReceipt(receipt AgentOutputReceipt) error {
	if err := validateReceiptPayload(receipt); err != nil {
		return err
	}
	if err := requireDigest("receipt_sha256", receipt.ReceiptSHA256); err != nil {
		return err
	}
	digest, err := receiptDigest(receipt)
	if err != nil {
		return err
	}
	if digest != receipt.ReceiptSHA256 {
		return fmt.Errorf("output binding: receipt_sha256 mismatch")
	}
	return nil
}

func validateReceiptPayload(receipt AgentOutputReceipt) error {
	if receipt.APIVersion != receiptAPI || receipt.Canonicalization != canonicalization ||
		receipt.Kind != receiptKind || receipt.ProfileID != localProfile ||
		receipt.SourceStateProfile != sourceStateProfile {
		return fmt.Errorf("output binding: receipt fixed fields drifted")
	}
	if receipt.LedgerSequence < 1 || receipt.LedgerSequence > maxSequence ||
		receipt.Attempt < 1 || receipt.Attempt > maxSequence ||
		receipt.ObservedAtUnixMS < 0 || receipt.ObservedAtUnixMS > maxSequence {
		return fmt.Errorf("output binding: receipt sequence, attempt, or observed time is invalid")
	}
	if err := validatePriorLinkShape(receipt); err != nil {
		return err
	}
	if err := validateReceiptText(receipt); err != nil {
		return err
	}
	if err := validateReceiptOutput(receipt); err != nil {
		return err
	}
	if err := validateReceiptNested(receipt); err != nil {
		return err
	}
	return validateReceiptPreflight(receipt)
}

func validatePriorLinkShape(receipt AgentOutputReceipt) error {
	if receipt.LedgerSequence == 1 {
		if receipt.PriorReceiptSHA256 != nil {
			return fmt.Errorf("output binding: genesis receipt prior_receipt_sha256 must be null")
		}
		return nil
	}
	if receipt.PriorReceiptSHA256 == nil {
		return fmt.Errorf("output binding: non-genesis receipt requires prior_receipt_sha256")
	}
	return requireDigest("prior_receipt_sha256", *receipt.PriorReceiptSHA256)
}

func validateReceiptText(receipt AgentOutputReceipt) error {
	fields := map[string]string{
		"agent": receipt.Agent, "model": receipt.Model, "phase": receipt.Phase,
		"run_id": receipt.RunID, "workflow": receipt.Workflow,
	}
	for label, value := range fields {
		if err := validateIdentifier(label, value); err != nil {
			return fmt.Errorf("output binding: receipt: %w", err)
		}
	}
	if err := validateWireText(receipt.Executor, false, maxReferenceBytes); err != nil {
		return fmt.Errorf("output binding: receipt executor: %w", err)
	}
	if !validSourceRevision(receipt.SourceRevision) {
		return fmt.Errorf("output binding: receipt source_revision is invalid")
	}
	if receipt.Verdict != nil && !oneOf(*receipt.Verdict, "APPROVE", "REQUEST_CHANGES") {
		return fmt.Errorf("output binding: receipt verdict is invalid")
	}
	if receipt.Verdict != nil &&
		(!receipt.RuntimePolicy.Reviewer || receipt.RuntimePolicy.VerdictContract != "reviewer_v2") {
		return fmt.Errorf("output binding: receipt verdict requires reviewer_v2 policy")
	}
	return nil
}

func validateReceiptOutput(receipt AgentOutputReceipt) error {
	if receipt.RawOutputBytes < 0 || receipt.RawOutputBytes > maxOutputBytes ||
		receipt.SemanticOutputBytes < 0 || receipt.SemanticOutputBytes > maxOutputBytes {
		return fmt.Errorf("output binding: raw and semantic output byte counts must be in 0..%d", maxOutputBytes)
	}
	fields := map[string]string{
		"binding_sha256": receipt.BindingSHA256, "challenge": receipt.Challenge,
		"final_prompt_sha256":    receipt.FinalPromptSHA256,
		"prompt_context_sha256":  receipt.PromptContextSHA256,
		"raw_output_sha256":      receipt.RawOutputSHA256,
		"semantic_output_sha256": receipt.SemanticOutputSHA256,
		"source_after_sha256":    receipt.SourceAfterSHA256,
		"source_before_sha256":   receipt.SourceBeforeSHA256,
	}
	for label, value := range fields {
		if err := requireDigest("receipt "+label, value); err != nil {
			return err
		}
	}
	if receipt.RawOutputBytes == 0 && receipt.RawOutputSHA256 != SHA256(nil) {
		return fmt.Errorf("output binding: zero-byte raw output must use the SHA-256 of empty bytes")
	}
	if receipt.SemanticOutputBytes == 0 && receipt.SemanticOutputSHA256 != SHA256(nil) {
		return fmt.Errorf("output binding: zero-byte semantic output must use the SHA-256 of empty bytes")
	}
	return nil
}

func validateReceiptNested(receipt AgentOutputReceipt) error {
	if err := ValidateRuntimePolicy(receipt.RuntimePolicy); err != nil {
		return fmt.Errorf("output binding: receipt runtime_policy: %w", err)
	}
	if err := ValidateManifest(receipt.ArtifactInputs); err != nil {
		return fmt.Errorf("output binding: receipt artifact_inputs: %w", err)
	}
	if err := ValidateManifest(receipt.ArtifactOutputs); err != nil {
		return fmt.Errorf("output binding: receipt artifact_outputs: %w", err)
	}
	if receipt.LocalRuntimePolicySHA256 != receipt.RuntimePolicy.BindingSHA256 ||
		receipt.ArtifactInputsSHA256 != receipt.ArtifactInputs.ManifestSHA256 ||
		receipt.ArtifactOutputsSHA256 != receipt.ArtifactOutputs.ManifestSHA256 {
		return fmt.Errorf("output binding: receipt nested digest reference mismatch")
	}
	if receipt.Agent != receipt.RuntimePolicy.Agent || receipt.Phase != receipt.RuntimePolicy.Phase ||
		receipt.Model != receipt.RuntimePolicy.Model ||
		receipt.Executor != receipt.RuntimePolicy.Executor {
		return fmt.Errorf("output binding: receipt runtime policy identity mismatch")
	}
	return nil
}

func validateReceiptPreflight(receipt AgentOutputReceipt) error {
	binding := PreflightBinding{
		ArtifactInputsSHA256: receipt.ArtifactInputsSHA256, Attempt: receipt.Attempt,
		Challenge: receipt.Challenge, LocalRuntimePolicySHA256: receipt.LocalRuntimePolicySHA256,
		Phase: receipt.Phase, PromptContextSHA256: receipt.PromptContextSHA256,
		RunID: receipt.RunID, SourceBeforeSHA256: receipt.SourceBeforeSHA256,
		Workflow: receipt.Workflow, WorkflowSHA256: receipt.RuntimePolicy.WorkflowSHA256,
	}
	sealed, err := SealPreflight(binding)
	if err != nil {
		return fmt.Errorf("output binding: reconstruct receipt preflight: %w", err)
	}
	if sealed.BindingSHA256 != receipt.BindingSHA256 {
		return fmt.Errorf("output binding: receipt binding_sha256 does not match reconstructed preflight")
	}
	return nil
}

func receiptDigest(receipt AgentOutputReceipt) (string, error) {
	receipt.ReceiptSHA256 = ""
	encoded, err := canonicalJSON(receipt, maxReceiptBytes)
	if err != nil {
		return "", err
	}
	return domainDigest(receiptDomain, encoded), nil
}

// CanonicalReceiptJSON returns exact compact canonical bytes without an LF.
func CanonicalReceiptJSON(receipt AgentOutputReceipt) ([]byte, error) {
	if err := ValidateReceipt(receipt); err != nil {
		return nil, err
	}
	return canonicalJSON(receipt, maxReceiptBytes)
}

// DecodeCanonicalReceipt accepts only the exact v1 canonical wire form.
func DecodeCanonicalReceipt(data []byte) (AgentOutputReceipt, error) {
	var receipt AgentOutputReceipt
	err := decodeExact(data, maxReceiptBytes, &receipt,
		func() error { return ValidateReceipt(receipt) },
		func() ([]byte, error) { return canonicalJSON(receipt, maxReceiptBytes) })
	return receipt, err
}

func cloneReceipt(receipt AgentOutputReceipt) AgentOutputReceipt {
	receipt.ArtifactInputs.Items = cloneManifestItems(receipt.ArtifactInputs.Items)
	receipt.ArtifactOutputs.Items = cloneManifestItems(receipt.ArtifactOutputs.Items)
	receipt.RuntimePolicy.Gates = cloneStrings(receipt.RuntimePolicy.Gates)
	receipt.PriorReceiptSHA256 = cloneString(receipt.PriorReceiptSHA256)
	receipt.Verdict = cloneString(receipt.Verdict)
	return receipt
}

func cloneManifestItems(items []ManifestItem) []ManifestItem {
	if items == nil {
		return nil
	}
	cloned := make([]ManifestItem, len(items))
	copy(cloned, items)
	return cloned
}

func cloneStrings(values []string) []string {
	if values == nil {
		return nil
	}
	cloned := make([]string, len(values))
	copy(cloned, values)
	return cloned
}

func cloneString(value *string) *string {
	if value == nil {
		return nil
	}
	cloned := *value
	return &cloned
}
