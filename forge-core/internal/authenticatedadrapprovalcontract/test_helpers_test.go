package authenticatedadrapprovalcontract

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"os"
	"path/filepath"
	"testing"
)

func repositoryPath(parts ...string) string {
	prefix := []string{"..", "..", ".."}
	return filepath.Join(append(prefix, parts...)...)
}

func loadGoldenRaw(t *testing.T) []byte {
	t.Helper()
	raw, err := os.ReadFile(repositoryPath("docs", "contracts", "fixtures",
		"authenticated-architecture-decision-approval-v1.json"))
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.HasSuffix(raw, []byte("\n")) || bytes.HasSuffix(raw, []byte("\n\n")) {
		t.Fatal("golden does not have exactly one physical LF")
	}
	return raw
}

func loadGoldenBundle(t *testing.T) (*Bundle, []byte) {
	t.Helper()
	raw := loadGoldenRaw(t)
	bundle, err := DecodeCanonicalBundle(raw[:len(raw)-1])
	if err != nil {
		t.Fatal(err)
	}
	return bundle, raw[:len(raw)-1]
}

func loadProposal(t *testing.T) []byte {
	t.Helper()
	raw, err := os.ReadFile(repositoryPath("docs", "contracts", "fixtures",
		"ADR-9002-authenticated-approval-target.md"))
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func loadSchema(t *testing.T) []byte {
	t.Helper()
	raw, err := os.ReadFile(repositoryPath("docs", "contracts",
		"authenticated-architecture-decision-approval-v1.schema.json"))
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func goldenRoot(t *testing.T, bundle *Bundle) *TrustRoot {
	t.Helper()
	raw, err := boundedCanonicalJSON(bundle.document["trust_root"], maxRootBytes, "root")
	if err != nil {
		t.Fatal(err)
	}
	root, err := DecodeCanonicalTrustRoot(raw)
	if err != nil {
		t.Fatal(err)
	}
	return root
}

func goldenInput(t *testing.T, bundle *Bundle, root *TrustRoot) *AuthorizationInput {
	t.Helper()
	policy, err := boundedCanonicalJSON(bundle.document["authorization_policy"], maxPolicyBytes, "policy")
	if err != nil {
		t.Fatal(err)
	}
	request, err := boundedCanonicalJSON(bundle.document["authorization_request"], maxRequestBytes, "request")
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err := boundedCanonicalJSON(bundle.document["revocation_snapshot"], maxRevocationBytes, "snapshot")
	if err != nil {
		t.Fatal(err)
	}
	input, err := DecodeAuthorizationInput(EncodedAuthorizationInput{
		ProposalDocument: loadProposal(t), Policy: policy,
		RevocationSnapshots: [][]byte{snapshot}, Request: request,
	}, root)
	if err != nil {
		t.Fatal(err)
	}
	return input
}

func goldenLedger(t *testing.T, bundle *Bundle, root *TrustRoot) *Ledger {
	t.Helper()
	raw, err := boundedCanonicalJSON(bundle.document["authorization_ledger"], maxLedgerBytes, "ledger")
	if err != nil {
		t.Fatal(err)
	}
	ledger, err := DecodeCanonicalLedger(raw, root)
	if err != nil {
		t.Fatal(err)
	}
	return ledger
}

func physicalSHA256(raw []byte) string {
	digest := sha256.Sum256(raw)
	return hex.EncodeToString(digest[:])
}

func resealPolicy(t *testing.T, node map[string]any) {
	t.Helper()
	node["policy_sha256"] = ""
	digest, err := policySHA256(node)
	if err != nil {
		t.Fatal(err)
	}
	node["policy_sha256"] = digest
}

func resealRevocation(t *testing.T, node map[string]any) {
	t.Helper()
	node["revocation_sha256"] = ""
	digest, err := revocationSHA256(node)
	if err != nil {
		t.Fatal(err)
	}
	node["revocation_sha256"] = digest
}

func resealLedger(t *testing.T, node map[string]any) {
	t.Helper()
	node["ledger_sha256"] = ""
	digest, err := ledgerSHA256(node)
	if err != nil {
		t.Fatal(err)
	}
	node["ledger_sha256"] = digest
}
