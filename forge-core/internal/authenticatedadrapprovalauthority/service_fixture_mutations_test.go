package authenticatedadrapprovalauthority

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"testing"
)

const testProposalBindingDomain = "forgeos.authenticated-architecture-decision-approval.proposal-binding.v1\x00"

func (f *serviceFixture) appendRevocation(t *testing.T,
	revokedApprovals, revokedKeys []string) {
	t.Helper()
	prior := f.revocations[len(f.revocations)-1]
	next := cloneTestObject(prior)
	next["prior_revocation_sha256"] = prior["revocation_sha256"]
	next["revocation_sequence"] = prior["revocation_sequence"].(int64) + 1
	next["revoked_approval_ids"] = mergeTestStrings(
		prior["revoked_approval_ids"].([]any), revokedApprovals)
	next["revoked_key_ids"] = mergeTestStrings(prior["revoked_key_ids"].([]any), revokedKeys)
	sealFixtureRevocation(t, f, next)
	f.revocations = append(f.revocations, next)
	f.request["revocation_sequence"] = next["revocation_sequence"]
	f.request["revocation_sha256"] = next["revocation_sha256"]
}

func sealFixtureRevocation(t *testing.T, fixture *serviceFixture, node map[string]any) {
	t.Helper()
	digest := testSelfDigest(t, testRevocationDomain, node,
		[]string{"revocation_sha256"}, true)
	node["revocation_sha256"] = digest
	testSignNode(t, node, fixture.private[fixture.byUsage["approval_revocation_sign"]],
		testRevokeSignDomain, digest)
}

func mergeTestStrings(prior []any, additional []string) []any {
	seen := map[string]bool{}
	for _, value := range prior {
		seen[value.(string)] = true
	}
	for _, value := range additional {
		seen[value] = true
	}
	values := make([]string, 0, len(seen))
	for value := range seen {
		values = append(values, value)
	}
	sortStringsForTest(values)
	result := make([]any, len(values))
	for index, value := range values {
		result[index] = value
	}
	return result
}

func sortStringsForTest(values []string) {
	for index := 1; index < len(values); index++ {
		for cursor := index; cursor > 0 && values[cursor] < values[cursor-1]; cursor-- {
			values[cursor], values[cursor-1] = values[cursor-1], values[cursor]
		}
	}
}

func (f *serviceFixture) advanceRequest(t *testing.T, sequence int64,
	ledgerSHA256 *string, idempotencyKey string) {
	t.Helper()
	f.request["expected_next_sequence"] = sequence
	if ledgerSHA256 == nil {
		f.request["expected_ledger_sha256"] = nil
	} else {
		f.request["expected_ledger_sha256"] = *ledgerSHA256
	}
	f.request["idempotency_key"] = idempotencyKey
	f.sealRequest(t)
}

func (f *serviceFixture) retargetProposal(t *testing.T, adrID, documentName string) {
	t.Helper()
	frontmatter, body := splitTestADR(t, f.proposal)
	oldID := frontmatter["adr_id"].(string)
	body = bytes.Replace(body, []byte(oldID), []byte(adrID), 1)
	frontmatter["adr_id"], frontmatter["document_name"] = adrID, documentName
	frontmatter["body_sha256"] = testADRDigest(
		"forgeos.architecture-decision-record-body.v2", body)
	frontmatter["self_sha256"] = ""
	blank := testCanonical(t, frontmatter)
	frontmatter["self_sha256"] = testADRDigest(
		"forgeos.architecture-decision-record.v2", blank, body)
	front := testCanonical(t, frontmatter)
	f.proposal = bytes.Join([][]byte{[]byte("---\n"), front, []byte("\n---\n\n"), body}, nil)
	f.rebindProposal(t, adrID, documentName, frontmatter)
}

func splitTestADR(t *testing.T, raw []byte) (map[string]any, []byte) {
	t.Helper()
	if !bytes.HasPrefix(raw, []byte("---\n")) {
		t.Fatal("proposal has no frontmatter opener")
	}
	separator := []byte("\n---\n\n")
	index := bytes.Index(raw[4:], separator)
	if index < 0 {
		t.Fatal("proposal has no frontmatter closer")
	}
	index += 4
	frontmatter, err := decodeTestObject(raw[4:index])
	if err != nil {
		t.Fatal(err)
	}
	return frontmatter, append([]byte(nil), raw[index+len(separator):]...)
}

func (f *serviceFixture) rebindProposal(t *testing.T, adrID, documentName string,
	frontmatter map[string]any) {
	t.Helper()
	physical := sha256.Sum256(f.proposal)
	binding := cloneTestObject(f.policy["proposal_binding"])
	binding["adr_id"], binding["document_name"] = adrID, documentName
	binding["body_sha256"] = frontmatter["body_sha256"]
	binding["self_sha256"] = frontmatter["self_sha256"]
	binding["physical_sha256"] = hex.EncodeToString(physical[:])
	binding["proposal_binding_sha256"] = testSelfDigest(t, testProposalBindingDomain,
		binding, []string{"proposal_binding_sha256"}, false)
	f.policy["proposal_binding"] = binding
	f.request["proposal_binding"] = cloneTestObject(binding)
	for _, item := range f.request["approval_records"].([]any) {
		rebindApprovalArtifacts(item.(map[string]any), binding)
	}
	f.sealPolicy(t)
	f.sealApprovals(t)
	f.sealRequest(t)
}

func rebindApprovalArtifacts(record, binding map[string]any) {
	artifacts := record["bindings"].(map[string]any)["artifacts"].([]any)
	for _, item := range artifacts {
		artifact := item.(map[string]any)
		switch artifact["artifact_kind"] {
		case "architecture-decision-proposal-body-v2":
			artifact["artifact_ref"] = binding["document_name"].(string) + "#body"
			artifact["artifact_sha256"] = binding["body_sha256"]
		case "architecture-decision-proposal-physical-v2":
			artifact["artifact_ref"] = binding["document_name"]
			artifact["artifact_sha256"] = binding["physical_sha256"]
		case "architecture-decision-proposal-self-v2":
			artifact["artifact_ref"] = binding["adr_id"]
			artifact["artifact_sha256"] = binding["self_sha256"]
		default:
			panic(fmt.Sprintf("unknown proposal artifact %v", artifact["artifact_kind"]))
		}
	}
	testSortNodes(artifacts)
}

func testADRDigest(domain string, parts ...[]byte) string {
	hasher := sha256.New()
	_, _ = hasher.Write([]byte(domain))
	for _, part := range parts {
		_, _ = hasher.Write([]byte{0})
		_, _ = hasher.Write(part)
	}
	return hex.EncodeToString(hasher.Sum(nil))
}
