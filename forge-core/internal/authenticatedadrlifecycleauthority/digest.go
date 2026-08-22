package authenticatedadrlifecycleauthority

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
)

func selfDigest(domain string, value map[string]any, fields []string,
	maximum int, label string, signed bool) (string, error) {
	if _, err := canonicalJSON(value, maximum, label); err != nil {
		return "", err
	}
	payload := cloneValue(value).(map[string]any)
	for _, field := range fields {
		payload[field] = ""
	}
	if signed {
		signature, err := objectValue(payload["signature"], label+".signature")
		if err != nil {
			return "", err
		}
		signature["signature_base64url"] = ""
	}
	encoded, err := canonicalJSON(payload, maximum, label+" digest preimage")
	if err != nil {
		return "", err
	}
	return domainDigest(domain, encoded), nil
}

func domainDigest(domain string, payload []byte) string {
	hasher := sha256.New()
	_, _ = hasher.Write([]byte(domain))
	_, _ = hasher.Write(payload)
	return hex.EncodeToString(hasher.Sum(nil))
}

func sha256Bytes(value []byte) string {
	digest := sha256.Sum256(value)
	return hex.EncodeToString(digest[:])
}

func signatureMessage(domain, digestHex string) ([]byte, error) {
	if domain == "" || domain[len(domain)-1] != 0 {
		return nil, fmt.Errorf("signature domain must end in NUL")
	}
	digest, err := hex.DecodeString(digestHex)
	if err != nil || len(digest) != 32 || len(digestHex) != 64 {
		return nil, fmt.Errorf("signature digest is invalid")
	}
	return append(append([]byte(nil), []byte(domain)...), digest...), nil
}

func recordKeySHA256(value string) (string, error) {
	if !validIdempotency(value) {
		return "", fmt.Errorf("idempotency key has invalid grammar")
	}
	return domainDigest(recordKeyDomain, []byte(value)), nil
}

func headSetSHA256(decisions []any) (string, error) {
	heads := make([]any, 0)
	for _, raw := range decisions {
		decision, err := objectValue(raw, "decision")
		if err != nil {
			return "", err
		}
		status, err := stringField(decision, "status")
		if err != nil {
			return "", err
		}
		if status != "accepted" {
			continue
		}
		adrID, err := stringField(decision, "adr_id")
		if err != nil {
			return "", err
		}
		binding, err := stringField(decision, "proposal_binding_sha256")
		if err != nil {
			return "", err
		}
		heads = append(heads, map[string]any{"adr_id": adrID, "proposal_binding_sha256": binding})
	}
	sortByADRID(heads)
	encoded, err := canonicalJSON(heads, 8*1024*1024, "current head set")
	if err != nil {
		return "", err
	}
	return domainDigest(headDomain, encoded), nil
}

func digestFor(kind string, node map[string]any) (string, error) {
	switch kind {
	case "prerequisite":
		return selfDigest(prerequisiteDomain, node, []string{"prerequisite_sha256"}, maxRequest, kind, false)
	case "request":
		return selfDigest(requestDigestDomain, node, []string{"request_id", "request_sha256"}, maxRequest, kind, true)
	case "acceptance":
		return selfDigest(acceptanceDomain, node, []string{"acceptance_id", "acceptance_sha256"}, 256*1024, kind, true)
	case "supersession":
		return selfDigest(supersessionDomain, node, []string{"receipt_id", "receipt_sha256"}, 256*1024, kind, true)
	case "entry":
		return selfDigest(entryDomain, node, []string{"entry_sha256"}, 4*1024*1024, kind, false)
	case "ledger":
		return selfDigest(ledgerDomain, node, []string{"ledger_sha256"}, 64*1024*1024, kind, false)
	case "view":
		return selfDigest(viewDomain, node, []string{"view_sha256"}, 8*1024*1024, kind, false)
	case "state":
		return selfDigest(stateDomain, node, []string{"state_sha256"}, int(maxState), kind, true)
	case "root":
		return selfDigest(rootDomain, node, []string{"root_sha256"}, int(maxRoot), kind, false)
	case "profile":
		return selfDigest(profileDomain, node, []string{"profile_sha256"}, int(maxProfile), kind, false)
	default:
		return "", fmt.Errorf("unknown digest kind %q", kind)
	}
}
