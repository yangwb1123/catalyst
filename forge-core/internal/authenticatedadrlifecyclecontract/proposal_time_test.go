package authenticatedadrlifecyclecontract

import (
	"bytes"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"testing"
)

const adrSelfDigestDomain = "forgeos.architecture-decision-record.v2"

func TestSourceProposalHalfOpenExpiryAndClockFreeHead(t *testing.T) {
	node := goldenNode(t)
	entry := lifecycleEntries(node)[2]
	accepted := entry["acceptance_receipt"].(map[string]any)["accepted_at_unix_ms"].(int64)
	request := entry["request"].(map[string]any)
	receipt := request["acceptance_prerequisite"].(map[string]any)["authorization_receipt"].(map[string]any)
	receipt["authorization_expires_at_unix_ms"] = accepted + 1
	request["expires_at_unix_ms"] = accepted + 1
	replaceProposalTime(t, node, 2, "expires_at_unix_ms", accepted+1)
	bundle := requireAccepted(t, node)
	facts, err := StructuralFacts(bundle)
	if err != nil {
		t.Fatal(err)
	}
	if facts.Decisions[2].ExpiresAtUnixMS == nil || *facts.Decisions[2].ExpiresAtUnixMS != accepted+1 ||
		facts.Decisions[2].Status != "accepted" {
		t.Fatal("structural head unexpectedly applied an ambient current-time judgment")
	}

	node = goldenNode(t)
	entry = lifecycleEntries(node)[2]
	accepted = entry["acceptance_receipt"].(map[string]any)["accepted_at_unix_ms"].(int64)
	replaceProposalTime(t, node, 2, "expires_at_unix_ms", accepted)
	requireRejected(t, node)
}

func TestSourceProposalAndApprovalTimeBoundsRejectFullyResealed(t *testing.T) {
	node := goldenNode(t)
	entry := lifecycleEntries(node)[2]
	accepted := entry["acceptance_receipt"].(map[string]any)["accepted_at_unix_ms"].(int64)
	replaceProposalTime(t, node, 2, "proposed_at_unix_ms", accepted+1)
	requireRejected(t, node)

	node = goldenNode(t)
	entry = lifecycleEntries(node)[2]
	request := entry["request"].(map[string]any)
	receipt := request["acceptance_prerequisite"].(map[string]any)["authorization_receipt"].(map[string]any)
	evaluated := receipt["evaluated_at_unix_ms"].(int64)
	replaceProposalTime(t, node, 2, "proposed_at_unix_ms", evaluated+1)
	requireRejected(t, node)

	node = goldenNode(t)
	entry = lifecycleEntries(node)[2]
	request = entry["request"].(map[string]any)
	receipt = request["acceptance_prerequisite"].(map[string]any)["authorization_receipt"].(map[string]any)
	authorizationExpires := receipt["authorization_expires_at_unix_ms"].(int64)
	replaceProposalTime(t, node, 2, "expires_at_unix_ms", authorizationExpires-1)
	requireRejected(t, node)
}

func replaceProposalTime(t *testing.T, node map[string]any, entryIndex int,
	field string, value int64) {
	t.Helper()
	entry := lifecycleEntries(node)[entryIndex]
	request := entry["request"].(map[string]any)
	raw, err := base64.RawURLEncoding.Strict().DecodeString(
		request["proposal_document_base64url"].(string))
	if err != nil {
		t.Fatal(err)
	}
	frontmatter, body := splitADRForTest(t, raw)
	frontmatter[field] = value
	frontmatter["self_sha256"] = ""
	blanked, err := boundedCanonicalJSON(frontmatter, 64*1024, "test ADR frontmatter")
	if err != nil {
		t.Fatal(err)
	}
	frontmatter["self_sha256"] = adrDomainDigestForTest(adrSelfDigestDomain, blanked, body)
	sealed, err := boundedCanonicalJSON(frontmatter, 64*1024, "test ADR frontmatter")
	if err != nil {
		t.Fatal(err)
	}
	changed := append([]byte("---\n"), sealed...)
	changed = append(changed, []byte("\n---\n\n")...)
	changed = append(changed, body...)
	binding, _, err := deriveProposalBinding(changed, frontmatter["document_name"].(string))
	if err != nil {
		t.Fatal(err)
	}
	prerequisite := request["acceptance_prerequisite"].(map[string]any)
	prerequisite["proposal_binding"] = binding
	prerequisite["authorization_receipt"].(map[string]any)["proposal_binding_sha256"] =
		binding["proposal_binding_sha256"]
	request["proposal_document_base64url"] = base64.RawURLEncoding.EncodeToString(changed)
	acceptance := entry["acceptance_receipt"].(map[string]any)
	acceptance["proposal_binding_sha256"] = binding["proposal_binding_sha256"]
	resealCascade(t, node, entryIndex, false)
}

func splitADRForTest(t *testing.T, raw []byte) (map[string]any, []byte) {
	t.Helper()
	if !bytes.HasPrefix(raw, []byte("---\n")) {
		t.Fatal("test ADR lacks prefix")
	}
	remainder := raw[4:]
	separator := []byte("\n---\n\n")
	index := bytes.Index(remainder, separator)
	if index < 0 {
		t.Fatal("test ADR lacks separator")
	}
	value, err := parseStrictJSON(remainder[:index], 64*1024)
	if err != nil {
		t.Fatal(err)
	}
	return value.(map[string]any), append([]byte(nil), remainder[index+len(separator):]...)
}

func adrDomainDigestForTest(domain string, parts ...[]byte) string {
	hasher := sha256.New()
	_, _ = hasher.Write([]byte(domain))
	for _, part := range parts {
		_, _ = hasher.Write([]byte{0})
		_, _ = hasher.Write(part)
	}
	return hex.EncodeToString(hasher.Sum(nil))
}
