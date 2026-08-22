package authenticatedadrlifecyclecontract

import (
	"fmt"
	"sort"

	approvalcontract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

func validateLedger(value any, profileHash string, lifecycleRoot map[string]any,
	approvalRoot *approvalcontract.TrustRoot) (map[string]any, map[string]map[string]any, error) {
	label := "ArchitectureDecisionLifecycleLedger"
	fields := []string{"api_version", "canonicalization", "current_head_set_sha256", "entries",
		"kind", "last_sequence", "ledger_sha256", "profile_id"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, nil, err
	}
	if _, err = boundedCanonicalJSON(node, maxLedgerBytes, label); err != nil {
		return nil, nil, err
	}
	if err = validateLedgerEnvelope(node); err != nil {
		return nil, nil, err
	}
	entries, err := arrayValue(node["entries"], "ledger.entries", 1, maxEntries)
	if err != nil {
		return nil, nil, err
	}
	state := make(map[string]map[string]any)
	position := ledgerValidationPosition{recordKeys: map[string]bool{}, approvalReceipts: map[string]bool{}}
	for index, rawEntry := range entries {
		entry, metadata, entryErr := validateEntryShape(rawEntry, profileHash,
			lifecycleRoot, approvalRoot)
		if entryErr != nil {
			return nil, nil, entryErr
		}
		if entryErr = validateEntryCAS(entry, int64(index+1), position, state); entryErr != nil {
			return nil, nil, entryErr
		}
		if entryErr = applyEntry(entry, metadata, state, &position); entryErr != nil {
			return nil, nil, entryErr
		}
		position.previousEntry = entry
		position.previousLedger, entryErr = prefixLedger(node, entries[:index+1], state)
		if entryErr != nil {
			return nil, nil, entryErr
		}
	}
	if err = validateFinalLedger(node, position.previousLedger, state); err != nil {
		return nil, nil, err
	}
	return node, state, nil
}

type ledgerValidationPosition struct {
	previousEntry    map[string]any
	previousLedger   map[string]any
	previousTime     *int64
	recordKeys       map[string]bool
	approvalReceipts map[string]bool
}

func validateLedgerEnvelope(node map[string]any) error {
	if node["api_version"] != ledgerAPI || node["canonicalization"] != canonicalization ||
		node["kind"] != "ArchitectureDecisionLifecycleLedger" || node["profile_id"] != profileID {
		return fmt.Errorf("lifecycle ledger envelope drifted from v1")
	}
	if _, err := intValue(node["last_sequence"], "ledger.last_sequence", 1, maxEntries); err != nil {
		return err
	}
	for _, field := range []string{"current_head_set_sha256", "ledger_sha256"} {
		if _, err := shaValue(node[field], "ledger."+field); err != nil {
			return err
		}
	}
	return nil
}

func validateEntryCAS(entry map[string]any, sequence int64, position ledgerValidationPosition,
	state map[string]map[string]any) error {
	request := entry["request"].(map[string]any)
	var priorEntry, priorLedger any
	if position.previousEntry != nil {
		priorEntry = position.previousEntry["entry_sha256"]
		priorLedger = position.previousLedger["ledger_sha256"]
	}
	if entry["sequence"] != sequence || request["expected_next_sequence"] != sequence {
		return fmt.Errorf("lifecycle ledger sequence is not contiguous")
	}
	if entry["prior_entry_sha256"] != priorEntry {
		return fmt.Errorf("lifecycle entry prior digest chain is broken")
	}
	if request["expected_ledger_sha256"] != priorLedger {
		return fmt.Errorf("request expected ledger digest differs from exact prefix")
	}
	head, err := currentHeadSetSHA256(state)
	if err != nil || request["expected_current_head_set_sha256"] != head {
		return fmt.Errorf("request expected current-head set differs from rebuilt prefix")
	}
	return nil
}

func prefixLedger(template map[string]any, entries []any,
	state map[string]map[string]any) (map[string]any, error) {
	prefix := cloneValue(template).(map[string]any)
	prefix["entries"] = cloneValue(entries)
	prefix["last_sequence"] = int64(len(entries))
	head, err := currentHeadSetSHA256(state)
	if err != nil {
		return nil, err
	}
	prefix["current_head_set_sha256"] = head
	prefix["ledger_sha256"] = ""
	digest, err := ledgerSHA256(prefix)
	if err != nil {
		return nil, err
	}
	prefix["ledger_sha256"] = digest
	return prefix, nil
}

func validateFinalLedger(node, rebuilt map[string]any,
	state map[string]map[string]any) error {
	entries := node["entries"].([]any)
	if node["last_sequence"] != int64(len(entries)) {
		return fmt.Errorf("ledger last_sequence differs from complete entries")
	}
	head, err := currentHeadSetSHA256(state)
	if err != nil || node["current_head_set_sha256"] != head {
		return fmt.Errorf("ledger current-head set differs from rebuilt state")
	}
	digest, err := ledgerSHA256(node)
	if err != nil || node["ledger_sha256"] != digest {
		return fmt.Errorf("lifecycle ledger self digest does not match")
	}
	if rebuilt == nil || rebuilt["ledger_sha256"] != node["ledger_sha256"] {
		return fmt.Errorf("complete lifecycle ledger differs from final prefix")
	}
	return nil
}

func currentHeadSetSHA256(state map[string]map[string]any) (string, error) {
	heads := make([]any, 0)
	for _, item := range state {
		if item["status"] == "accepted" {
			heads = append(heads, map[string]any{"adr_id": item["adr_id"],
				"proposal_binding_sha256": item["proposal_binding_sha256"]})
		}
	}
	sortNodesByADRID(heads)
	return digestValue(headDomain, heads, maxViewBytes, "structural current-head set")
}

func sortNodesByADRID(nodes []any) {
	sort.Slice(nodes, func(left, right int) bool {
		return nodes[left].(map[string]any)["adr_id"].(string) <
			nodes[right].(map[string]any)["adr_id"].(string)
	})
}
