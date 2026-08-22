package authenticatedadrlifecycleauthority

import (
	"fmt"
	"sort"
)

type prospectiveState struct {
	bundleNode map[string]any
	stateJSON  []byte
	resultJSON []byte
	sequence   int64
}

func buildProspective(input preparedInput, prior *authenticatedState,
	material authorityMaterial, signer wireSigner, verifyProofs bool) (prospectiveState, error) {
	var result prospectiveState
	entries, decisions, err := priorCollections(prior)
	if err != nil {
		return result, err
	}
	state, err := buildStateImage(input, prior, material, signer, entries, decisions)
	if err != nil {
		return result, err
	}
	delivery, _, err := resultForState(state, input.sequence, "stored")
	if err != nil {
		return result, err
	}
	node := bundleNode(material, state, delivery)
	disposition := "placeholder"
	if verifyProofs {
		disposition = "stored"
	}
	_, stateJSON, resultJSON, err := validateBundleNode(node, material, disposition)
	if err != nil {
		return result, err
	}
	return prospectiveState{bundleNode: node, stateJSON: stateJSON,
		resultJSON: resultJSON, sequence: input.sequence}, nil
}

func buildStateImage(input preparedInput, prior *authenticatedState,
	material authorityMaterial, signer wireSigner, entries,
	decisions []any) (map[string]any, error) {
	if err := requireFreshPosition(input, prior, entries, decisions); err != nil {
		return nil, err
	}
	acceptance, err := buildAcceptance(input, material.stateKey, signer)
	if err != nil {
		return nil, err
	}
	supersessions, err := buildSupersessions(input, acceptance, material.stateKey, signer)
	if err != nil {
		return nil, err
	}
	updated, err := updateDecisions(input, decisions, acceptance, supersessions)
	if err != nil {
		return nil, err
	}
	head, err := headSetSHA256(updated)
	if err != nil {
		return nil, err
	}
	var priorEntry any
	if len(entries) > 0 {
		previous, itemErr := objectValue(entries[len(entries)-1], "prior entry")
		if itemErr != nil {
			return nil, itemErr
		}
		priorEntry = previous["entry_sha256"]
	}
	entry, err := buildEntry(input, acceptance, supersessions, priorEntry, head)
	if err != nil {
		return nil, err
	}
	entries = append(entries, entry)
	ledger, err := buildLedger(entries, head)
	if err != nil {
		return nil, err
	}
	view, err := buildView(ledger, updated)
	if err != nil {
		return nil, err
	}
	return buildState(ledger, view, material, signer)
}

func priorCollections(prior *authenticatedState) ([]any, []any, error) {
	if prior == nil {
		return make([]any, 0), make([]any, 0), nil
	}
	entries, err := arrayField(prior.ledger, "entries")
	if err != nil {
		return nil, nil, err
	}
	decisions, err := arrayField(prior.view, "decisions")
	if err != nil {
		return nil, nil, err
	}
	return cloneValue(entries).([]any), cloneValue(decisions).([]any), nil
}

func requireFreshPosition(input preparedInput, prior *authenticatedState,
	entries, decisions []any) error {
	if len(entries) >= maxEntries || len(decisions) >= maxDecisions {
		return coded(codeCapacityExhausted, fmt.Errorf("lifecycle state capacity reached"))
	}
	if input.sequence != int64(len(entries)+1) {
		return coded(codeCASConflict, fmt.Errorf("expected next sequence differs"))
	}
	var expectedLedger any
	if prior != nil {
		expectedLedger = prior.ledger["ledger_sha256"]
	}
	if input.request["expected_ledger_sha256"] != expectedLedger {
		return coded(codeCASConflict, fmt.Errorf("expected ledger digest differs"))
	}
	head, err := headSetSHA256(decisions)
	if err != nil {
		return err
	}
	if input.request["expected_current_head_set_sha256"] != head {
		return coded(codeCASConflict, fmt.Errorf("expected head set differs"))
	}
	for _, raw := range decisions {
		decision, itemErr := objectValue(raw, "decision")
		if itemErr != nil {
			return itemErr
		}
		if decision["adr_id"] == input.proposalID {
			return coded(codeInputRejected, fmt.Errorf("ADR already has lifecycle state"))
		}
	}
	return requireTargetBindings(input.request, decisions)
}

func updateDecisions(input preparedInput, decisions []any, acceptance map[string]any,
	supersessions []any) ([]any, error) {
	byID := map[string]map[string]any{}
	for _, raw := range decisions {
		decision, err := objectValue(raw, "decision")
		if err != nil {
			return nil, err
		}
		adrID, err := stringField(decision, "adr_id")
		if err != nil {
			return nil, err
		}
		byID[adrID] = decision
	}
	for _, raw := range supersessions {
		receipt, err := objectValue(raw, "supersession receipt")
		if err != nil {
			return nil, err
		}
		targetID, err := stringField(receipt, "target_adr_id")
		if err != nil {
			return nil, err
		}
		target := byID[targetID]
		if target == nil {
			return nil, fmt.Errorf("supersession target is absent")
		}
		target["status"] = "superseded"
		target["superseded_at_unix_ms"] = input.observed
		target["superseded_by"] = []any{input.proposalID}
		target["supersession_receipt_sha256"] = receipt["receipt_sha256"]
	}
	newDecision, err := newDecision(input, acceptance)
	if err != nil {
		return nil, err
	}
	byID[input.proposalID] = newDecision
	identifiers := make([]string, 0, len(byID))
	for identifier := range byID {
		identifiers = append(identifiers, identifier)
	}
	sort.Strings(identifiers)
	result := make([]any, len(identifiers))
	for index, identifier := range identifiers {
		result[index] = byID[identifier]
	}
	return result, nil
}

func newDecision(input preparedInput, acceptance map[string]any) (map[string]any, error) {
	prerequisite, err := objectField(input.request, "acceptance_prerequisite")
	if err != nil {
		return nil, err
	}
	receipt, err := objectField(prerequisite, "authorization_receipt")
	if err != nil {
		return nil, err
	}
	targets, err := targetIDs(input.request)
	if err != nil {
		return nil, err
	}
	return map[string]any{
		"acceptance_id": acceptance["acceptance_id"], "acceptance_sha256": acceptance["acceptance_sha256"],
		"accepted_at_unix_ms": input.observed, "adr_id": input.proposalID,
		"authorization_receipt_physical_sha256": prerequisite["authorization_receipt_physical_sha256"],
		"authorization_receipt_sha256":          receipt["receipt_sha256"],
		"document_name":                         input.document.Frontmatter.DocumentName,
		"expires_at_unix_ms":                    nullableInt(input.document.Frontmatter.ExpiresAtUnixMS),
		"proposal_binding_sha256":               input.proposalHash,
		"proposed_at_unix_ms":                   input.document.Frontmatter.ProposedAtUnixMS,
		"source_body_sha256":                    input.document.Frontmatter.BodySHA256,
		"source_physical_sha256":                sha256Bytes(input.source.ProposalDocument),
		"source_self_sha256":                    input.document.Frontmatter.SelfSHA256, "status": "accepted",
		"superseded_at_unix_ms": nil, "superseded_by": []any{},
		"supersession_receipt_sha256": nil, "supersedes": stringsToAny(targets),
	}, nil
}

func buildLedger(entries []any, head string) (map[string]any, error) {
	value := map[string]any{"api_version": ledgerAPI, "canonicalization": canonicalization,
		"current_head_set_sha256": head, "entries": entries,
		"kind": "ArchitectureDecisionLifecycleLedger", "last_sequence": int64(len(entries)),
		"ledger_sha256": "", "profile_id": profileID}
	digest, err := digestFor("ledger", value)
	if err != nil {
		return nil, err
	}
	value["ledger_sha256"] = digest
	return value, nil
}

func buildView(ledger map[string]any, decisions []any) (map[string]any, error) {
	heads := make([]any, 0)
	for _, raw := range decisions {
		decision, err := objectValue(raw, "decision")
		if err != nil {
			return nil, err
		}
		if decision["status"] == "accepted" {
			heads = append(heads, decision["adr_id"])
		}
	}
	value := map[string]any{"api_version": viewAPI, "canonicalization": canonicalization,
		"current_head_set_sha256": ledger["current_head_set_sha256"], "decisions": decisions,
		"head_adr_ids": heads, "kind": "ArchitectureDecisionLifecycleMaterializedView",
		"last_sequence": ledger["last_sequence"], "ledger_sha256": ledger["ledger_sha256"],
		"profile_id": profileID, "view_sha256": ""}
	digest, err := digestFor("view", value)
	if err != nil {
		return nil, err
	}
	value["view_sha256"] = digest
	return value, nil
}

func buildState(ledger, view map[string]any, material authorityMaterial,
	signer wireSigner) (map[string]any, error) {
	value := map[string]any{"api_version": stateAPI, "canonicalization": canonicalization,
		"kind": "ArchitectureDecisionLifecycleState", "ledger": ledger,
		"materialized_view": view, "profile_id": profileID,
		"signature": signatureNode(material.stateKey.KeyID, ""), "state_sha256": "",
		"trust_epoch": material.lifecycle["trust_epoch"], "trust_root_sha256": material.lifecycle["root_sha256"]}
	digest, err := digestFor("state", value)
	if err != nil {
		return nil, err
	}
	value["state_sha256"] = digest
	proof, err := signer.sign(stateSignDomain, digest)
	if err != nil {
		return nil, err
	}
	value["signature"] = signatureNode(material.stateKey.KeyID, proof)
	return value, nil
}

func nullableInt(value *int64) any {
	if value == nil {
		return nil
	}
	return *value
}
