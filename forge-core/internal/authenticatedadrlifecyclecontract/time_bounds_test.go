package authenticatedadrlifecyclecontract

import (
	"strings"
	"testing"
)

func TestApprovalProjectionNAndNPlusOne(t *testing.T) {
	node := goldenNode(t)
	prerequisite := finalPrerequisite(node)
	prerequisite["authorization_ledger_last_sequence"] = int64(64)
	prerequisite["revocation_high_water_sequence"] = int64(256)
	prerequisite["revocation_high_water_sha256"] = strings.Repeat("e", 64)
	resealCascade(t, node, 2, false)
	requireAccepted(t, node)

	for field, value := range map[string]int64{
		"authorization_ledger_last_sequence": 65,
		"revocation_high_water_sequence":     257,
	} {
		t.Run(field, func(t *testing.T) {
			candidate := goldenNode(t)
			finalPrerequisite(candidate)[field] = value
			resealCascade(t, candidate, 2, false)
			requireRejected(t, candidate)
		})
	}
}

func TestApprovalClockAndPriorLinkRelations(t *testing.T) {
	node := goldenNode(t)
	prerequisite := finalPrerequisite(node)
	evaluated := prerequisite["authorization_receipt"].(map[string]any)["evaluated_at_unix_ms"].(int64)
	prerequisite["authorization_ledger_clock_high_water_unix_ms"] = evaluated
	resealCascade(t, node, 2, false)
	requireAccepted(t, node)

	node = goldenNode(t)
	prerequisite = finalPrerequisite(node)
	evaluated = prerequisite["authorization_receipt"].(map[string]any)["evaluated_at_unix_ms"].(int64)
	prerequisite["authorization_ledger_clock_high_water_unix_ms"] = evaluated - 1
	resealCascade(t, node, 2, false)
	requireRejected(t, node)

	node = goldenNode(t)
	finalPrerequisite(node)["authorization_receipt"].(map[string]any)["prior_receipt_sha256"] = nil
	resealCascade(t, node, 2, false)
	requireRejected(t, node)

	node = goldenNode(t)
	second := lifecycleEntries(node)[1]["request"].(map[string]any)["acceptance_prerequisite"].(map[string]any)
	second["authorization_receipt"].(map[string]any)["prior_receipt_sha256"] = strings.Repeat("d", 64)
	resealCascade(t, node, 1, false)
	requireRejected(t, node)
}

func TestRequestValidityNAndNPlusOne(t *testing.T) {
	node := goldenNode(t)
	request := lifecycleEntries(node)[2]["request"].(map[string]any)
	requested := request["requested_at_unix_ms"].(int64)
	if request["expires_at_unix_ms"] != requested+maxRequestValidityMS {
		t.Fatal("golden does not exercise exact request validity N")
	}
	requireAccepted(t, node)

	node = goldenNode(t)
	request = lifecycleEntries(node)[2]["request"].(map[string]any)
	requested = request["requested_at_unix_ms"].(int64)
	request["expires_at_unix_ms"] = requested + maxRequestValidityMS + 1
	receipt := request["acceptance_prerequisite"].(map[string]any)["authorization_receipt"].(map[string]any)
	receipt["authorization_expires_at_unix_ms"] = request["expires_at_unix_ms"]
	resealCascade(t, node, 2, false)
	requireRejected(t, node)
}

func TestExactObservationAndNondecreasingLifecycleTime(t *testing.T) {
	node := goldenNode(t)
	entry := lifecycleEntries(node)[2]
	entry["acceptance_receipt"].(map[string]any)["accepted_at_unix_ms"] =
		entry["acceptance_receipt"].(map[string]any)["accepted_at_unix_ms"].(int64) + 1
	resealCascade(t, node, 2, false)
	requireRejected(t, node)

	node = goldenNode(t)
	entries := lifecycleEntries(node)
	observed := entries[0]["acceptance_receipt"].(map[string]any)["accepted_at_unix_ms"].(int64) - 1
	entry = entries[1]
	request := entry["request"].(map[string]any)
	prerequisite := request["acceptance_prerequisite"].(map[string]any)
	receipt := prerequisite["authorization_receipt"].(map[string]any)
	prerequisite["observed_at_unix_ms"] = observed
	prerequisite["authorization_ledger_clock_high_water_unix_ms"] = observed
	receipt["evaluated_at_unix_ms"] = observed - 100
	receipt["authorization_expires_at_unix_ms"] = observed + maxRequestValidityMS
	request["requested_at_unix_ms"] = observed
	request["expires_at_unix_ms"] = observed + maxRequestValidityMS
	entry["acceptance_receipt"].(map[string]any)["accepted_at_unix_ms"] = observed
	resealCascade(t, node, 1, false)
	requireRejected(t, node)
}

func finalPrerequisite(node map[string]any) map[string]any {
	entries := lifecycleEntries(node)
	return entries[len(entries)-1]["request"].(map[string]any)["acceptance_prerequisite"].(map[string]any)
}
