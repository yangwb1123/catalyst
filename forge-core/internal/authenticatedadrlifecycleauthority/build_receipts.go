package authenticatedadrlifecycleauthority

import "fmt"

func signatureNode(keyID, proof string) map[string]any {
	return map[string]any{"key_id": keyID, "profile_id": signatureProfile,
		"profile_sha256": profileSHA256, "signature_base64url": proof}
}

func buildAcceptance(input preparedInput, key lifecycleKey,
	signer wireSigner) (map[string]any, error) {
	prerequisite, err := objectField(input.request, "acceptance_prerequisite")
	if err != nil {
		return nil, err
	}
	receipt, err := objectField(prerequisite, "authorization_receipt")
	if err != nil {
		return nil, err
	}
	requestSHA, err := stringField(input.request, "request_sha256")
	if err != nil {
		return nil, err
	}
	recordKey, err := recordKeySHA256(input.idempotency)
	if err != nil {
		return nil, err
	}
	targetIDs, err := targetIDs(input.request)
	if err != nil {
		return nil, err
	}
	value := map[string]any{
		"acceptance_id": "", "acceptance_sha256": "", "accepted_at_unix_ms": input.observed,
		"adr_id": input.proposalID, "api_version": acceptanceAPI,
		"authorization_receipt_physical_sha256": prerequisite["authorization_receipt_physical_sha256"],
		"authorization_receipt_sha256":          receipt["receipt_sha256"], "canonicalization": canonicalization,
		"kind": "ArchitectureDecisionLifecycleAcceptanceReceipt", "ledger_sequence": input.sequence,
		"profile_id": profileID, "proposal_binding_sha256": input.proposalHash,
		"record_key_sha256": recordKey, "request_sha256": requestSHA,
		"signature": signatureNode(key.KeyID, ""), "supersedes": stringsToAny(targetIDs),
		"trust_epoch": input.request["trust_epoch"], "trust_root_sha256": input.request["trust_root_sha256"],
	}
	digest, err := digestFor("acceptance", value)
	if err != nil {
		return nil, err
	}
	value["acceptance_sha256"] = digest
	value["acceptance_id"] = "architecture-decision-acceptance-" + digest
	proof, err := signer.sign(acceptanceSignDomain, digest)
	if err != nil {
		return nil, err
	}
	value["signature"] = signatureNode(key.KeyID, proof)
	return value, nil
}

func buildSupersessions(input preparedInput, acceptance map[string]any,
	key lifecycleKey, signer wireSigner) ([]any, error) {
	targets, err := arrayField(input.request, "supersession_targets")
	if err != nil {
		return nil, err
	}
	result := make([]any, len(targets))
	for index, raw := range targets {
		target, itemErr := objectValue(raw, "supersession target")
		if itemErr != nil {
			return nil, itemErr
		}
		value := map[string]any{
			"api_version": supersessionAPI, "canonicalization": canonicalization,
			"kind":            "ArchitectureDecisionLifecycleSupersessionReceipt",
			"ledger_sequence": input.sequence, "profile_id": profileID,
			"receipt_id": "", "receipt_sha256": "", "request_sha256": input.request["request_sha256"],
			"signature": signatureNode(key.KeyID, ""), "superseded_at_unix_ms": input.observed,
			"superseded_by_acceptance_id":           acceptance["acceptance_id"],
			"superseded_by_adr_id":                  input.proposalID,
			"superseded_by_proposal_binding_sha256": input.proposalHash,
			"target_acceptance_id":                  target["acceptance_id"], "target_adr_id": target["adr_id"],
			"target_proposal_binding_sha256": target["proposal_binding_sha256"],
			"trust_epoch":                    input.request["trust_epoch"], "trust_root_sha256": input.request["trust_root_sha256"],
		}
		digest, itemErr := digestFor("supersession", value)
		if itemErr != nil {
			return nil, itemErr
		}
		value["receipt_sha256"] = digest
		value["receipt_id"] = "architecture-decision-supersession-" + digest
		proof, itemErr := signer.sign(supersessionSign, digest)
		if itemErr != nil {
			return nil, itemErr
		}
		value["signature"] = signatureNode(key.KeyID, proof)
		result[index] = value
	}
	return result, nil
}

func buildEntry(input preparedInput, acceptance map[string]any,
	supersessions []any, priorEntry any, resultingHead string) (map[string]any, error) {
	value := map[string]any{
		"acceptance_receipt": acceptance, "api_version": entryAPI,
		"canonicalization": canonicalization, "entry_sha256": "",
		"kind": "ArchitectureDecisionLifecycleLedgerEntry", "prior_entry_sha256": priorEntry,
		"profile_id": profileID, "request": cloneValue(input.request),
		"resulting_current_head_set_sha256": resultingHead, "sequence": input.sequence,
		"supersession_receipts": supersessions,
	}
	digest, err := digestFor("entry", value)
	if err != nil {
		return nil, err
	}
	value["entry_sha256"] = digest
	return value, nil
}

func stringsToAny(values []string) []any {
	result := make([]any, len(values))
	for index, value := range values {
		result[index] = value
	}
	return result
}

func requireTargetBindings(request map[string]any, decisions []any) error {
	byID := map[string]map[string]any{}
	for _, raw := range decisions {
		decision, err := objectValue(raw, "decision")
		if err != nil {
			return err
		}
		adrID, err := stringField(decision, "adr_id")
		if err != nil {
			return err
		}
		byID[adrID] = decision
	}
	targets, err := arrayField(request, "supersession_targets")
	if err != nil {
		return err
	}
	for _, raw := range targets {
		target, itemErr := objectValue(raw, "target")
		if itemErr != nil {
			return itemErr
		}
		adrID, itemErr := stringField(target, "adr_id")
		if itemErr != nil {
			return itemErr
		}
		decision := byID[adrID]
		if decision == nil || decision["status"] != "accepted" ||
			decision["acceptance_id"] != target["acceptance_id"] ||
			decision["acceptance_sha256"] != target["acceptance_sha256"] ||
			decision["proposal_binding_sha256"] != target["proposal_binding_sha256"] {
			return fmt.Errorf("supersession target %s is not the exact current row", adrID)
		}
	}
	return nil
}
