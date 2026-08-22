package authenticatedadrapprovalcontract

import (
	"fmt"
	"regexp"
)

var approvalIDPattern = regexp.MustCompile(`^approval-record-[0-9a-f]{64}$`)

func validateRevocation(value any, root map[string]any) (map[string]any, error) {
	label := "ArchitectureDecisionApprovalRevocationSnapshot"
	fields := []string{"api_version", "canonicalization", "effective_at_unix_ms",
		"expires_at_unix_ms", "kind", "prior_revocation_sha256", "profile_id",
		"revocation_sequence", "revocation_sha256", "revoked_approval_ids",
		"revoked_key_ids", "signature", "trust_epoch", "trust_root_sha256"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, err
	}
	if _, err = boundedCanonicalJSON(node, maxRevocationBytes, label); err != nil {
		return nil, err
	}
	if node["api_version"] != revocationAPI || node["canonicalization"] != canonicalization ||
		node["kind"] != label || node["profile_id"] != profileID {
		return nil, fmt.Errorf("%s envelope drifted from v1", label)
	}
	sequence, err := intValue(node["revocation_sequence"], "revocation.revocation_sequence", 1, maxInt64)
	if err != nil {
		return nil, err
	}
	if err = validateRevocationTime(node); err != nil {
		return nil, err
	}
	if err = validateRevokedValues(node, root); err != nil {
		return nil, err
	}
	if err = validateRevocationAuthority(node, root); err != nil {
		return nil, err
	}
	if sequence == 1 && node["prior_revocation_sha256"] != nil {
		return nil, fmt.Errorf("first revocation snapshot must have null prior digest")
	}
	if sequence > 1 {
		if _, err = shaValue(node["prior_revocation_sha256"], "revocation.prior_revocation_sha256"); err != nil {
			return nil, err
		}
	}
	digest, err := revocationSHA256(node)
	if err != nil || node["revocation_sha256"] != digest {
		return nil, fmt.Errorf("revocation snapshot self digest does not match")
	}
	return node, nil
}

func validateRevocationTime(node map[string]any) error {
	effective, err := intValue(node["effective_at_unix_ms"], "revocation.effective_at_unix_ms", 0, maxInt64)
	if err != nil {
		return err
	}
	expires, err := intValue(node["expires_at_unix_ms"], "revocation.expires_at_unix_ms", 0, maxInt64)
	if err != nil {
		return err
	}
	if effective >= expires || expires-effective > maxPolicyValidityMS {
		return fmt.Errorf("revocation snapshot validity must be ordered within 24 hours")
	}
	return nil
}

func validateRevokedValues(node, root map[string]any) error {
	approvals, err := sortedUniqueStringValues(node["revoked_approval_ids"], "revocation.revoked_approval_ids", 0, 256)
	if err != nil {
		return err
	}
	for _, approvalID := range approvals {
		if !approvalIDPattern.MatchString(approvalID) {
			return fmt.Errorf("revoked approval ID is malformed")
		}
	}
	keyIDs, err := sortedUniqueStringValues(node["revoked_key_ids"], "revocation.revoked_key_ids", 0, 20)
	if err != nil {
		return err
	}
	for _, keyID := range keyIDs {
		if _, err = keyNodeByID(root, keyID); err != nil {
			return err
		}
	}
	return nil
}

func validateRevocationAuthority(node, root map[string]any) error {
	if node["trust_root_sha256"] != root["root_sha256"] || node["trust_epoch"] != root["trust_epoch"] {
		return fmt.Errorf("revocation snapshot does not bind the trust root")
	}
	if _, err := intValue(node["trust_epoch"], "revocation.trust_epoch", 1, maxInt64); err != nil {
		return err
	}
	signature, err := validateSignature(node["signature"], "revocation.signature", signatureProfileSHA256Pin)
	if err != nil {
		return err
	}
	key, err := keyNodeForUsage(root, "approval_revocation_sign")
	if err != nil || signature["key_id"] != key["key_id"] {
		return fmt.Errorf("revocation signature uses the wrong root key usage")
	}
	if arrayContains(node["revoked_key_ids"].([]any), key["key_id"].(string)) {
		return fmt.Errorf("revocation snapshot cannot revoke its own signing key")
	}
	return nil
}

func validateRevocationChain(value any, root map[string]any) ([]map[string]any, error) {
	items, err := arrayValue(value, "ledger.revocation_snapshots", 1, maxRevocationSnapshots)
	if err != nil {
		return nil, err
	}
	result := make([]map[string]any, len(items))
	var prior any
	priorApprovals, priorKeys := map[string]bool{}, map[string]bool{}
	priorEffective := int64(-1)
	for index, item := range items {
		node, validateErr := validateRevocation(item, root)
		if validateErr != nil {
			return nil, validateErr
		}
		if node["revocation_sequence"] != int64(index+1) || node["prior_revocation_sha256"] != prior {
			return nil, fmt.Errorf("revocation sequence and prior digest chain must be contiguous")
		}
		if node["effective_at_unix_ms"].(int64) < priorEffective {
			return nil, fmt.Errorf("revocation effective times must be nondecreasing")
		}
		approvals := stringSet(node["revoked_approval_ids"].([]any))
		keys := stringSet(node["revoked_key_ids"].([]any))
		if !setContains(approvals, priorApprovals) || !setContains(keys, priorKeys) {
			return nil, fmt.Errorf("revocation sets must be monotonic within one root epoch")
		}
		prior = node["revocation_sha256"]
		priorApprovals, priorKeys = approvals, keys
		priorEffective, result[index] = node["effective_at_unix_ms"].(int64), node
	}
	return result, nil
}

func stringSet(values []any) map[string]bool {
	result := make(map[string]bool, len(values))
	for _, value := range values {
		result[value.(string)] = true
	}
	return result
}

func setContains(superset, subset map[string]bool) bool {
	for value := range subset {
		if !superset[value] {
			return false
		}
	}
	return true
}
