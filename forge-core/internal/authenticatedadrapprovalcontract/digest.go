package authenticatedadrapprovalcontract

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
	digest := sha256.Sum256(append([]byte(domain), encoded...))
	return hex.EncodeToString(digest[:]), nil
}

func recordKeySHA256(idempotencyKey string) (string, error) {
	if !idempotencyPattern.MatchString(idempotencyKey) {
		return "", fmt.Errorf("idempotency key must be 16..128 closed visible ASCII bytes")
	}
	digest := sha256.Sum256(append([]byte(recordKeyDomain), []byte(idempotencyKey)...))
	return hex.EncodeToString(digest[:]), nil
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
		"ArchitectureDecisionApprovalTrustRoot", false)
}

func proposalBindingSHA256(value map[string]any) (string, error) {
	return selfDigest(proposalBindingDomain, value, []string{"proposal_binding_sha256"},
		maxProposalBindingBytes, "ArchitectureDecisionProposalBinding", false)
}

func policySHA256(value map[string]any) (string, error) {
	return selfDigest(policyDomain, value, []string{"policy_sha256"}, maxPolicyBytes,
		"ArchitectureDecisionApprovalPolicy", true)
}

func revocationSHA256(value map[string]any) (string, error) {
	return selfDigest(revocationDomain, value, []string{"revocation_sha256"},
		maxRevocationBytes, "ArchitectureDecisionApprovalRevocationSnapshot", true)
}

func requestSHA256(value map[string]any) (string, error) {
	return selfDigest(requestDomain, value, []string{"request_id", "request_sha256"},
		maxRequestBytes, "ArchitectureDecisionApprovalAuthorizationRequest", true)
}

func receiptSHA256(value map[string]any) (string, error) {
	return selfDigest(receiptDomain, value, []string{"receipt_id", "receipt_sha256"},
		maxReceiptBytes, "ArchitectureDecisionApprovalAuthorizationReceipt", true)
}

func ledgerSHA256(value map[string]any) (string, error) {
	return selfDigest(ledgerDomain, value, []string{"ledger_sha256"}, maxLedgerBytes,
		"ArchitectureDecisionApprovalAuthorizationLedger", true)
}
