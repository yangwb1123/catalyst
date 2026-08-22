package authenticatedadrlifecyclecontract

import (
	"fmt"
	"sort"
)

func rebuildView(ledger map[string]any,
	state map[string]map[string]any) (map[string]any, error) {
	identifiers := make([]string, 0, len(state))
	for identifier := range state {
		identifiers = append(identifiers, identifier)
	}
	sort.Strings(identifiers)
	decisions := make([]any, len(identifiers))
	heads := make([]any, 0)
	for index, identifier := range identifiers {
		decision := state[identifier]
		decisions[index] = cloneValue(decision)
		if decision["status"] == "accepted" {
			heads = append(heads, identifier)
		}
	}
	view := map[string]any{
		"api_version": viewAPI, "canonicalization": canonicalization,
		"current_head_set_sha256": ledger["current_head_set_sha256"],
		"decisions":               decisions, "head_adr_ids": heads,
		"kind":          "ArchitectureDecisionLifecycleMaterializedView",
		"last_sequence": ledger["last_sequence"], "ledger_sha256": ledger["ledger_sha256"],
		"profile_id": profileID, "view_sha256": "",
	}
	digest, err := viewSHA256(view)
	if err != nil {
		return nil, err
	}
	view["view_sha256"] = digest
	return view, nil
}

func validateMaterializedView(value any, ledger map[string]any,
	state map[string]map[string]any) (map[string]any, error) {
	label := "ArchitectureDecisionLifecycleMaterializedView"
	fields := []string{"api_version", "canonicalization", "current_head_set_sha256", "decisions",
		"head_adr_ids", "kind", "last_sequence", "ledger_sha256", "profile_id", "view_sha256"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, err
	}
	if _, err = boundedCanonicalJSON(node, maxViewBytes, label); err != nil {
		return nil, err
	}
	if err = validateViewShape(node); err != nil {
		return nil, err
	}
	rebuilt, err := rebuildView(ledger, state)
	if err != nil || !canonicalEqual(node, rebuilt) {
		return nil, fmt.Errorf("materialized lifecycle view is not exact ledger rebuild")
	}
	return node, nil
}

func validateViewShape(node map[string]any) error {
	if node["api_version"] != viewAPI || node["canonicalization"] != canonicalization ||
		node["kind"] != "ArchitectureDecisionLifecycleMaterializedView" ||
		node["profile_id"] != profileID {
		return fmt.Errorf("materialized lifecycle view envelope drifted from v1")
	}
	if _, err := intValue(node["last_sequence"], "view.last_sequence", 1, maxEntries); err != nil {
		return err
	}
	for _, field := range []string{"current_head_set_sha256", "ledger_sha256", "view_sha256"} {
		if _, err := shaValue(node[field], "view."+field); err != nil {
			return err
		}
	}
	if err := validateDecisions(node["decisions"]); err != nil {
		return err
	}
	heads, err := sortedUniqueStrings(node["head_adr_ids"], "view.head_adr_ids", 0, maxDecisions)
	if err != nil {
		return err
	}
	for index, item := range heads {
		if _, err = adrIDValue(item, fmt.Sprintf("view.head_adr_ids[%d]", index)); err != nil {
			return err
		}
	}
	return nil
}

func validateDecisions(value any) error {
	items, err := arrayValue(value, "view.decisions", 1, maxDecisions)
	if err != nil {
		return err
	}
	prior := ""
	for index, item := range items {
		node, itemErr := validateDecisionShape(item, index)
		if itemErr != nil {
			return itemErr
		}
		identifier := node["adr_id"].(string)
		if index > 0 && prior >= identifier {
			return fmt.Errorf("materialized decisions must be sorted and unique by ADR ID")
		}
		prior = identifier
	}
	return nil
}

func validateDecisionShape(value any, index int) (map[string]any, error) {
	label := fmt.Sprintf("view.decisions[%d]", index)
	fields := []string{"acceptance_id", "acceptance_sha256", "accepted_at_unix_ms", "adr_id",
		"authorization_receipt_physical_sha256", "authorization_receipt_sha256", "document_name",
		"expires_at_unix_ms", "proposal_binding_sha256", "proposed_at_unix_ms",
		"source_body_sha256", "source_physical_sha256", "source_self_sha256", "status",
		"superseded_at_unix_ms", "superseded_by", "supersession_receipt_sha256", "supersedes"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, err
	}
	if _, err = adrIDValue(node["adr_id"], label+".adr_id"); err != nil {
		return nil, err
	}
	if _, err = enumValue(node["status"], label+".status", "accepted", "superseded"); err != nil {
		return nil, err
	}
	if _, err = sortedUniqueStrings(node["supersedes"], label+".supersedes", 0, maxDecisions); err != nil {
		return nil, err
	}
	if _, err = sortedUniqueStrings(node["superseded_by"], label+".superseded_by", 0, 1); err != nil {
		return nil, err
	}
	return node, nil
}
