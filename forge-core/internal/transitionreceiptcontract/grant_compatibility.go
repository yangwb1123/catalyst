package transitionreceiptcontract

import (
	"fmt"
	"sort"
	"strings"

	"forgeos/forge-core/internal/capabilitygrantcontract"
)

const grantCompatibilityResult = "ASSESSED_GRANT_TRANSITION_DECLARATIONS_ONLY (no permission or transition authority)"

// ProjectCapabilityGrantRef projects the ADR-0056 declared issuer domain and identity.
func ProjectCapabilityGrantRef(grant map[string]any) (map[string]any, error) {
	if _, err := capabilitygrantcontract.CanonicalGrantJSON(grant); err != nil {
		return nil, err
	}
	proof, ok := grant["authority_proof"].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("CapabilityGrant authority_proof is invalid")
	}
	issuer, ok := proof["issuer"].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("CapabilityGrant issuer is invalid")
	}
	return map[string]any{
		"authority_domain": issuer["authority_domain"], "grant_id": grant["grant_id"],
		"grant_sha256": grant["grant_sha256"],
	}, nil
}

// AssessDeclaredGrantCompatibility compares declared ADR-0056 fields only.
func AssessDeclaredGrantCompatibility(grant, receipt map[string]any) (map[string]any, error) {
	grantRef, err := ProjectCapabilityGrantRef(grant)
	if err != nil {
		return nil, err
	}
	if err := validateReceipt(receipt, false); err != nil {
		return nil, err
	}
	validity := grant["validity"].(map[string]any)
	declaredAt := receipt["transition"].(map[string]any)["declared_at_unix_ms"].(int64)
	timeMatches := validity["not_before_unix_ms"].(int64) <= declaredAt &&
		declaredAt < validity["expires_at_unix_ms"].(int64)
	relations := map[string]any{
		"actor": sameRelation(receipt["actor"], grant["subject"], "actor"),
		"approval_refs": sameRelation(receipt["approval_refs"], grant["approval_refs"],
			"approval_refs"),
		"bindings": sameRelation(receiptCompatibilityBindings(receipt),
			grantCompatibilityBindings(grant), "bindings"),
		"declared_time": relation(timeMatches, "same_declared_time", "declared_time_mismatch"),
		"grant_ref":     sameRelation(receipt["capability_grant_ref"], grantRef, "grant_ref"),
		"task_binding": sameRelation(receipt["task_binding"], grant["task_binding"],
			"task_binding"),
	}
	return compatibilityResult(relations, grantCompatibilityResult), nil
}

func grantCompatibilityBindings(grant map[string]any) map[string]any {
	return compatibilityBindings(grant["bindings"].(map[string]any))
}

func receiptCompatibilityBindings(receipt map[string]any) map[string]any {
	return compatibilityBindings(receipt["bindings"].(map[string]any))
}

func compatibilityBindings(bindings map[string]any) map[string]any {
	keys := []string{"context_sha256", "impact_sha256", "plan_sha256", "policy_sha256",
		"risk_sha256", "source_revision", "source_tree_sha256"}
	result := make(map[string]any, len(keys))
	for _, key := range keys {
		result[key] = cloneValue(bindings[key])
	}
	return result
}

func sameRelation(left, right any, field string) string {
	return relation(canonicalValuesEqual(left, right), "same_declared_"+field, field+"_mismatch")
}

func compatibilityResult(relations map[string]any, result string) map[string]any {
	reasons := make([]string, 0, len(relations))
	for _, value := range relations {
		text, ok := value.(string)
		if ok && strings.HasSuffix(text, "_mismatch") {
			reasons = append(reasons, text)
		}
	}
	sort.Strings(reasons)
	return map[string]any{
		"reason_codes": stringsToAny(reasons), "relations": relations, "result": result,
	}
}
