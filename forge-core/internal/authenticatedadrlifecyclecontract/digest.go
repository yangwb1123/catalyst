package authenticatedadrlifecyclecontract

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
)

func selfDigest(domain string, value map[string]any, fields []string,
	maximum int, label string, signed bool) (string, error) {
	if _, err := boundedCanonicalJSON(value, maximum, label); err != nil {
		return "", err
	}
	payload := cloneValue(value).(map[string]any)
	for _, field := range fields {
		payload[field] = ""
	}
	if signed {
		signature, ok := payload["signature"].(map[string]any)
		if !ok {
			return "", fmt.Errorf("%s has no closed signature object", label)
		}
		signature["signature_base64url"] = ""
	}
	encoded, err := boundedCanonicalJSON(payload, maximum, label+" digest preimage")
	if err != nil {
		return "", err
	}
	return domainDigest(domain, encoded), nil
}

func digestValue(domain string, value any, maximum int, label string) (string, error) {
	encoded, err := boundedCanonicalJSON(value, maximum, label)
	if err != nil {
		return "", err
	}
	return domainDigest(domain, encoded), nil
}

func domainDigest(domain string, payload []byte) string {
	digest := sha256.New()
	_, _ = digest.Write([]byte(domain))
	_, _ = digest.Write(payload)
	return hex.EncodeToString(digest.Sum(nil))
}

func sha256Bytes(value []byte) string {
	digest := sha256.Sum256(value)
	return hex.EncodeToString(digest[:])
}

func signatureMessage(domain, digestHex string) ([]byte, error) {
	if domain == "" || domain[len(domain)-1] != 0 {
		return nil, fmt.Errorf("signature domain must be NUL-terminated")
	}
	digest, err := decodeHexSHA(digestHex)
	if err != nil {
		return nil, err
	}
	return append(append([]byte(nil), []byte(domain)...), digest...), nil
}

func signatureProfileSHA256(value map[string]any) (string, error) {
	return selfDigest(signatureProfileDomain, value, []string{"profile_sha256"},
		maxProfileBytes, "SignatureProfile", false)
}

func trustRootSHA256(value map[string]any) (string, error) {
	return selfDigest(trustRootDomain, value, []string{"root_sha256"}, maxRootBytes,
		"ArchitectureDecisionLifecycleTrustRoot", false)
}

func proposalBindingSHA256(value map[string]any) (string, error) {
	return selfDigest(proposalBindingDomain, value, []string{"proposal_binding_sha256"},
		64*1024, "ArchitectureDecisionProposalBinding", false)
}

func prerequisiteSHA256(value map[string]any) (string, error) {
	return selfDigest(prerequisiteDomain, value, []string{"prerequisite_sha256"},
		maxRequestBytes, "ArchitectureDecisionAcceptancePrerequisite", false)
}

func requestSHA256(value map[string]any) (string, error) {
	return selfDigest(requestDomain, value, []string{"request_id", "request_sha256"},
		maxRequestBytes, "ArchitectureDecisionLifecycleTransitionRequest", true)
}

func acceptanceSHA256(value map[string]any) (string, error) {
	return selfDigest(acceptanceDomain, value, []string{"acceptance_id", "acceptance_sha256"},
		maxAcceptanceBytes, "ArchitectureDecisionLifecycleAcceptanceReceipt", true)
}

func supersessionSHA256(value map[string]any) (string, error) {
	return selfDigest(supersessionDomain, value, []string{"receipt_id", "receipt_sha256"},
		maxSupersessionBytes, "ArchitectureDecisionLifecycleSupersessionReceipt", true)
}

func entrySHA256(value map[string]any) (string, error) {
	return selfDigest(entryDomain, value, []string{"entry_sha256"}, maxEntryBytes,
		"ArchitectureDecisionLifecycleLedgerEntry", false)
}

func ledgerSHA256(value map[string]any) (string, error) {
	return selfDigest(ledgerDomain, value, []string{"ledger_sha256"}, maxLedgerBytes,
		"ArchitectureDecisionLifecycleLedger", false)
}

func viewSHA256(value map[string]any) (string, error) {
	return selfDigest(viewDomain, value, []string{"view_sha256"}, maxViewBytes,
		"ArchitectureDecisionLifecycleMaterializedView", false)
}

func stateSHA256(value map[string]any) (string, error) {
	return selfDigest(stateDomain, value, []string{"state_sha256"}, maxStateBytes,
		"ArchitectureDecisionLifecycleState", true)
}

func recordKeySHA256(idempotencyKey string) (string, error) {
	if !idempotencyPattern.MatchString(idempotencyKey) {
		return "", fmt.Errorf("idempotency key must be 16..128 closed visible ASCII bytes")
	}
	return domainDigest(recordKeyDomain, []byte(idempotencyKey)), nil
}
