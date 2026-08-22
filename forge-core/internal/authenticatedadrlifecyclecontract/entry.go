package authenticatedadrlifecyclecontract

import (
	"fmt"

	approvalcontract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

func validateEntryShape(value any, profileHash string, lifecycleRoot map[string]any,
	approvalRoot *approvalcontract.TrustRoot) (map[string]any, proposalMetadata, error) {
	label := "ArchitectureDecisionLifecycleLedgerEntry"
	fields := []string{"acceptance_receipt", "api_version", "canonicalization", "entry_sha256",
		"kind", "prior_entry_sha256", "profile_id", "request",
		"resulting_current_head_set_sha256", "sequence", "supersession_receipts"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, proposalMetadata{}, err
	}
	if _, err = boundedCanonicalJSON(node, maxEntryBytes, label); err != nil {
		return nil, proposalMetadata{}, err
	}
	if node["api_version"] != entryAPI || node["canonicalization"] != canonicalization ||
		node["kind"] != label || node["profile_id"] != profileID {
		return nil, proposalMetadata{}, fmt.Errorf("%s envelope drifted from v1", label)
	}
	sequence, err := intValue(node["sequence"], "entry.sequence", 1, maxInt64)
	if err != nil {
		return nil, proposalMetadata{}, err
	}
	if node["prior_entry_sha256"] != nil {
		if _, err = shaValue(node["prior_entry_sha256"], "entry.prior_entry_sha256"); err != nil {
			return nil, proposalMetadata{}, err
		}
	}
	if _, err = shaValue(node["resulting_current_head_set_sha256"],
		"entry.resulting_current_head_set_sha256"); err != nil {
		return nil, proposalMetadata{}, err
	}
	request, metadata, err := validateRequest(node["request"], profileHash,
		lifecycleRoot, approvalRoot)
	if err != nil {
		return nil, proposalMetadata{}, err
	}
	if err = validateEntryReceipts(node, request, sequence, profileHash, lifecycleRoot); err != nil {
		return nil, proposalMetadata{}, err
	}
	digest, err := entrySHA256(node)
	if err != nil || node["entry_sha256"] != digest {
		return nil, proposalMetadata{}, fmt.Errorf("lifecycle entry self digest does not match")
	}
	return node, metadata, nil
}

func validateEntryReceipts(node, request map[string]any, sequence int64,
	profileHash string, root map[string]any) error {
	if _, err := validateAcceptance(node["acceptance_receipt"], profileHash, root); err != nil {
		return err
	}
	items, err := arrayValue(node["supersession_receipts"],
		"entry.supersession_receipts", 0, maxSupersessions)
	if err != nil {
		return err
	}
	for _, item := range items {
		if _, err = validateSupersession(item, profileHash, root); err != nil {
			return err
		}
	}
	targets := request["supersession_targets"].([]any)
	if !receiptsFollowTargets(items, targets) {
		return fmt.Errorf("supersession receipts must exactly follow sorted targets")
	}
	if sequence != request["expected_next_sequence"] {
		return fmt.Errorf("entry sequence differs from request CAS sequence")
	}
	return nil
}

func receiptsFollowTargets(receipts, targets []any) bool {
	if len(receipts) != len(targets) {
		return false
	}
	for index := range receipts {
		receipt := receipts[index].(map[string]any)
		target := targets[index].(map[string]any)
		if receipt["target_adr_id"] != target["adr_id"] {
			return false
		}
	}
	return true
}
