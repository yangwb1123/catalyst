package kerneldecisioncontract

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"sort"

	op "forgeos/forge-core/internal/kerneloperationalcontract"
)

func validateAtomRef(value AtomRef, label string) error {
	if err := hash(value.AtomSHA256, label+".atom_sha256"); err != nil {
		return err
	}
	if value.AtomID != atomPrefix+value.AtomSHA256 {
		return fmt.Errorf("%s atom_id does not bind digest", label)
	}
	return nil
}

func validateAtomRefs(values []AtomRef, label string) error {
	if values == nil || len(values) > maxAtomRefs {
		return fmt.Errorf("%s cardinality must be 0..%d", label, maxAtomRefs)
	}
	ids := make([]string, len(values))
	for index, value := range values {
		if err := validateAtomRef(value, label); err != nil {
			return err
		}
		ids[index] = value.AtomID
	}
	if !sort.StringsAreSorted(ids) || duplicateStrings(ids) {
		return fmt.Errorf("%s must be strictly atom-id sorted and unique", label)
	}
	return nil
}

func validateBudget(value Budget) error {
	items := []struct {
		value, maximum int64
		label          string
	}{{value.MaxCalls, 1000000000, "max_calls"},
		{value.MaxCostUSDMicros, 1000000000000000, "max_cost_usd_micros"},
		{value.MaxInputTokens, 1000000000, "max_input_tokens"},
		{value.MaxNetworkBytes, 1073741824, "max_network_bytes"},
		{value.MaxOutputBytes, 1073741824, "max_output_bytes"},
		{value.MaxOutputTokens, 1000000000, "max_output_tokens"},
		{value.TimeoutMS, maxTimeoutMS, "timeout_ms"}}
	for _, item := range items {
		if item.value < 0 || item.value > item.maximum {
			return fmt.Errorf("budget.%s is outside its frozen range", item.label)
		}
	}
	if value.MaxCalls == 0 || value.TimeoutMS == 0 {
		return fmt.Errorf("budget max_calls and timeout_ms must be positive")
	}
	return nil
}

func validateCompensation(value Compensation) error {
	if !oneOf(value.Applicability, "not_applicable", "required") {
		return fmt.Errorf("compensation.applicability is unsupported")
	}
	capabilityPresent := value.Capability != nil
	actionPresent := value.RequestedActionSHA256 != nil
	if capabilityPresent != actionPresent || (value.Applicability == "required") != capabilityPresent {
		return fmt.Errorf("compensation members do not match applicability")
	}
	if capabilityPresent {
		if err := validateCapability(*value.Capability); err != nil {
			return err
		}
		return hash(*value.RequestedActionSHA256, "compensation.requested_action_sha256")
	}
	return nil
}

func validateOption(value DecisionOption) error {
	if err := validateCapability(value.Capability); err != nil {
		return err
	}
	if err := identifier(value.OptionID, "option_id"); err != nil {
		return err
	}
	return hash(value.RequestedActionSHA256, "requested_action_sha256")
}

func validateOptions(values []DecisionOption, selected string) error {
	if values == nil || len(values) == 0 || len(values) > maxOptions {
		return fmt.Errorf("options cardinality must be 1..%d", maxOptions)
	}
	ids := make([]string, len(values))
	found := false
	for index, value := range values {
		if err := validateOption(value); err != nil {
			return fmt.Errorf("options[%d]: %w", index, err)
		}
		ids[index] = value.OptionID
		found = found || value.OptionID == selected
	}
	if !sort.StringsAreSorted(ids) || duplicateStrings(ids) {
		return fmt.Errorf("options must be strictly option-id sorted and unique")
	}
	if !found {
		return fmt.Errorf("selected_option_id must identify one option")
	}
	return nil
}

func validateProofs(values []ProofObligation) error {
	if values == nil || len(values) == 0 || len(values) > maxProofs {
		return fmt.Errorf("proof_obligations cardinality must be 1..%d", maxProofs)
	}
	ids := make([]string, len(values))
	for index, value := range values {
		if err := identifier(value.ObligationID, "obligation_id"); err != nil {
			return err
		}
		if err := hash(value.PredicateSHA256, "predicate_sha256"); err != nil {
			return err
		}
		if err := validateStringSet(value.RequiredEvidenceKinds, "required_evidence_kinds",
			maxEvidenceKinds, true); err != nil {
			return err
		}
		ids[index] = value.ObligationID
	}
	if !sort.StringsAreSorted(ids) || duplicateStrings(ids) {
		return fmt.Errorf("proof_obligations must be strictly obligation-id sorted and unique")
	}
	return nil
}

func validateReceiptRefs(values []op.ArtifactReceiptRef) error {
	if values == nil || len(values) > maxIOItems {
		return fmt.Errorf("read_artifact_receipt_refs cardinality must be 0..%d", maxIOItems)
	}
	ids := make([]string, len(values))
	for index, value := range values {
		if err := identity(value.ArtifactReceiptID, value.ArtifactReceiptSHA256,
			"artifact-receipt-", "artifact_receipt", false); err != nil {
			return err
		}
		ids[index] = value.ArtifactReceiptID
	}
	if !sort.StringsAreSorted(ids) || duplicateStrings(ids) {
		return fmt.Errorf("read_artifact_receipt_refs must be strictly sorted and unique")
	}
	return nil
}

func validatePreconditions(values []WritePrecondition) error {
	if values == nil || len(values) > maxIOItems {
		return fmt.Errorf("write_preconditions cardinality must be 0..%d", maxIOItems)
	}
	ids := make([]string, len(values))
	for index, value := range values {
		if err := hash(value.ExpectedSHA256, "expected_sha256"); err != nil {
			return err
		}
		if err := identifier(value.PreconditionID, "precondition_id"); err != nil {
			return err
		}
		if err := text(value.ResourceRef, "resource_ref", maxSelectorBytes); err != nil {
			return err
		}
		ids[index] = value.PreconditionID
	}
	if !sort.StringsAreSorted(ids) || duplicateStrings(ids) {
		return fmt.Errorf("write_preconditions must be strictly sorted and unique")
	}
	return nil
}

func validateTransactionMembers(value *DecisionTransaction) error {
	if err := validateAttestations(value.Attestations); err != nil {
		return err
	}
	if err := validateBindings(value.Bindings); err != nil {
		return err
	}
	if err := validateTask(value.TaskBinding); err != nil {
		return err
	}
	if err := validatePrincipal(value.Actor, "actor"); err != nil {
		return err
	}
	if err := validatePrincipal(value.AccountableOwner, "accountable_owner"); err != nil {
		return err
	}
	if err := validateBudget(value.Budget); err != nil {
		return err
	}
	if err := text(value.CompletionCondition.ConditionRef, "condition_ref", maxSelectorBytes); err != nil {
		return err
	}
	if err := hash(value.CompletionCondition.ConditionSHA256, "condition_sha256"); err != nil {
		return err
	}
	return validateCompensation(value.Compensation)
}

func validateTransactionRoles(value *DecisionTransaction) error {
	if err := validateAtomRef(value.GoalAtomRef, "goal_atom_ref"); err != nil {
		return err
	}
	if err := validateAtomRefs(value.TriggerAtomRefs, "trigger_atom_refs"); err != nil {
		return err
	}
	if err := validateAtomRefs(value.GuardAtomRefs, "guard_atom_refs"); err != nil {
		return err
	}
	seen := map[string]bool{value.GoalAtomRef.AtomID: true}
	for _, ref := range append(append([]AtomRef{}, value.TriggerAtomRefs...), value.GuardAtomRefs...) {
		if seen[ref.AtomID] {
			return fmt.Errorf("goal, trigger and guard roles must be disjoint")
		}
		seen[ref.AtomID] = true
	}
	return nil
}

func validateVerifier(value *DecisionTransaction) error {
	if err := validateCapability(value.Verifier.Capability); err != nil {
		return err
	}
	if err := hash(value.Verifier.IndependenceBasisSHA256, "independence_basis_sha256"); err != nil {
		return err
	}
	if err := validatePrincipal(value.Verifier.Principal, "verifier.principal"); err != nil {
		return err
	}
	if value.Verifier.TimeoutMS <= 0 || value.Verifier.TimeoutMS > maxTimeoutMS {
		return fmt.Errorf("verifier.timeout_ms is outside frozen range")
	}
	if value.Verifier.Principal == value.Actor || value.Verifier.Principal == value.AccountableOwner {
		return fmt.Errorf("verifier principal must differ from actor and accountable owner")
	}
	return nil
}

func validateTransactionBody(value *DecisionTransaction, blank bool) error {
	if value == nil {
		return fmt.Errorf("DecisionTransaction is nil")
	}
	if value.APIVersion != transactionAPI || value.Canonicalization != canonicalization ||
		value.Kind != transactionKind || value.TransactionMode != "structural_proposal_only" {
		return fmt.Errorf("DecisionTransaction constants differ")
	}
	if err := identity(value.DecisionTransactionID, value.DecisionTransactionSHA256,
		transactionPrefix, "decision_transaction", blank); err != nil {
		return err
	}
	if value.CreatedAtUnixMS < 0 {
		return fmt.Errorf("created_at_unix_ms must be nonnegative")
	}
	if err := validateTransactionMembers(value); err != nil {
		return err
	}
	if err := validateTransactionRoles(value); err != nil {
		return err
	}
	return validateTransactionTail(value)
}

func validateTransactionTail(value *DecisionTransaction) error {
	if err := identifier(value.IdempotencyKey, "idempotency_key"); err != nil {
		return err
	}
	if err := validateOptions(value.Options, value.SelectedOptionID); err != nil {
		return err
	}
	if err := hash(value.SelectionBasisSHA256, "selection_basis_sha256"); err != nil {
		return err
	}
	if err := validateProofs(value.ProofObligations); err != nil {
		return err
	}
	if err := validateReceiptRefs(value.ReadArtifactReceiptRefs); err != nil {
		return err
	}
	if err := validateVerifier(value); err != nil {
		return err
	}
	if err := validatePreconditions(value.WritePreconditions); err != nil {
		return err
	}
	return validateStringSet(value.WriteSlots, "write_slots", maxIOItems, false)
}

func transactionDigest(value *DecisionTransaction) (string, error) {
	blank := *value
	blank.DecisionTransactionID, blank.DecisionTransactionSHA256 = "", ""
	if err := validateTransactionBody(&blank, true); err != nil {
		return "", err
	}
	raw, err := canonicalBytes(&blank, maxTransactionBytes)
	if err != nil {
		return "", err
	}
	digest := sha256.New()
	digest.Write(transactionDomain)
	digest.Write(raw)
	return hex.EncodeToString(digest.Sum(nil)), nil
}

func ValidateDecisionTransaction(value *DecisionTransaction) error {
	if err := validateTransactionBody(value, false); err != nil {
		return err
	}
	digest, err := transactionDigest(value)
	if err != nil {
		return err
	}
	if value.DecisionTransactionSHA256 != digest {
		return fmt.Errorf("decision_transaction_sha256 does not match canonical preimage")
	}
	_, err = canonicalBytes(value, maxTransactionBytes)
	return err
}

func SealDecisionTransaction(value *DecisionTransaction) (*DecisionTransaction, error) {
	if value == nil || value.DecisionTransactionID != "" || value.DecisionTransactionSHA256 != "" {
		return nil, fmt.Errorf("sealing DecisionTransaction requires blank identity")
	}
	sealed, err := cloneValue(value)
	if err != nil {
		return nil, err
	}
	digest, err := transactionDigest(sealed)
	if err != nil {
		return nil, err
	}
	sealed.DecisionTransactionID = transactionPrefix + digest
	sealed.DecisionTransactionSHA256 = digest
	return sealed, ValidateDecisionTransaction(sealed)
}

func DecodeDecisionTransaction(raw []byte) (*DecisionTransaction, error) {
	var value DecisionTransaction
	if err := decodeTyped(raw, maxTransactionBytes, &value); err != nil {
		return nil, err
	}
	return &value, ValidateDecisionTransaction(&value)
}
