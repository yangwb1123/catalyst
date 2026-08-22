package authenticatedadrlifecyclecontract

import (
	"fmt"

	approvalcontract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

// StructuralFacts returns detached ledger/view/result facts. They do not
// attest signature validity, time currentness, persistence, or authority.
func StructuralFacts(bundle *Bundle) (Facts, error) {
	if bundle == nil {
		return Facts{}, fmt.Errorf("bundle is nil")
	}
	node, context, err := validateDocument(bundle.document)
	if err != nil {
		return Facts{}, err
	}
	approvalFacts, err := approvalcontract.Facts(context.approvalRoot)
	if err != nil {
		return Facts{}, err
	}
	lifecycleRoot := context.lifecycleRoot
	state := node["lifecycle_state"].(map[string]any)
	ledger := state["ledger"].(map[string]any)
	view := state["materialized_view"].(map[string]any)
	result := Facts{ApprovalTrustRootSHA256: approvalFacts.TrustRootSHA256,
		ApprovalTrustEpoch:       approvalFacts.TrustEpoch,
		LifecycleTrustDomain:     lifecycleRoot["trust_domain"].(string),
		LifecycleTrustRootSHA256: lifecycleRoot["root_sha256"].(string),
		LifecycleTrustEpoch:      lifecycleRoot["trust_epoch"].(int64),
		LedgerSHA256:             ledger["ledger_sha256"].(string),
		LastSequence:             ledger["last_sequence"].(int64),
		CurrentHeadSetSHA256:     ledger["current_head_set_sha256"].(string),
		HeadADRIDs:               detachedStrings(view["head_adr_ids"].([]any)),
		ResultDisposition:        node["lifecycle_result"].(map[string]any)["delivery_disposition"].(string),
		StateSHA256:              state["state_sha256"].(string)}
	result.LifecycleRootKeys = lifecycleRootFacts(lifecycleRoot)
	result.Entries = entryFacts(ledger["entries"].([]any))
	result.Decisions = decisionFacts(view["decisions"].([]any))
	result.ResultSequence = resultSequence(node["lifecycle_result"].(map[string]any),
		ledger["entries"].([]any))
	return result, nil
}

func lifecycleRootFacts(root map[string]any) []RootKey {
	items := root["keys"].([]any)
	result := make([]RootKey, len(items))
	for index, item := range items {
		result[index] = lifecycleRootKeyView(item.(map[string]any))
	}
	return result
}

func entryFacts(entries []any) []EntryFact {
	result := make([]EntryFact, len(entries))
	for index, item := range entries {
		entry := item.(map[string]any)
		request := entry["request"].(map[string]any)
		prerequisite := request["acceptance_prerequisite"].(map[string]any)
		approval := prerequisite["authorization_receipt"].(map[string]any)
		acceptance := entry["acceptance_receipt"].(map[string]any)
		result[index] = EntryFact{Sequence: entry["sequence"].(int64),
			ApprovalReceiptSequence: approval["ledger_sequence"].(int64),
			ADRID:                   acceptance["adr_id"].(string), EntrySHA256: entry["entry_sha256"].(string),
			RequestSHA256:    request["request_sha256"].(string),
			AcceptanceSHA256: acceptance["acceptance_sha256"].(string),
			AcceptedAtUnixMS: acceptance["accepted_at_unix_ms"].(int64),
			TargetADRs:       targetStrings(request["supersession_targets"].([]any))}
	}
	return result
}

func targetStrings(targets []any) []string {
	result := make([]string, len(targets))
	for index, item := range targets {
		result[index] = item.(map[string]any)["adr_id"].(string)
	}
	return result
}

func decisionFacts(decisions []any) []DecisionFact {
	result := make([]DecisionFact, len(decisions))
	for index, item := range decisions {
		decision := item.(map[string]any)
		result[index] = DecisionFact{ADRID: decision["adr_id"].(string),
			AcceptanceID:          decision["acceptance_id"].(string),
			AcceptanceSHA256:      decision["acceptance_sha256"].(string),
			AcceptedAtUnixMS:      decision["accepted_at_unix_ms"].(int64),
			DocumentName:          decision["document_name"].(string),
			ExpiresAtUnixMS:       optionalInt64(decision["expires_at_unix_ms"]),
			ProposalBindingSHA256: decision["proposal_binding_sha256"].(string),
			SourcePhysicalSHA256:  decision["source_physical_sha256"].(string),
			Status:                decision["status"].(string),
			SupersededBy:          detachedStrings(decision["superseded_by"].([]any)),
			Supersedes:            detachedStrings(decision["supersedes"].([]any))}
	}
	return result
}

func optionalInt64(value any) *int64 {
	if value == nil {
		return nil
	}
	result := value.(int64)
	return &result
}

func detachedStrings(values []any) []string {
	result := make([]string, len(values))
	for index, value := range values {
		result[index] = value.(string)
	}
	return result
}

func resultSequence(result map[string]any, entries []any) int64 {
	for _, item := range entries {
		entry := item.(map[string]any)
		if entry["entry_sha256"] == result["entry_sha256"] {
			return entry["sequence"].(int64)
		}
	}
	return 0
}
