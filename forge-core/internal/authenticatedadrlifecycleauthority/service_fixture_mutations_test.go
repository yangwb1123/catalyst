//go:build unix && !aix && !solaris

package authenticatedadrlifecycleauthority

import (
	"bytes"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	approvalauthority "forgeos/forge-core/internal/authenticatedadrapprovalauthority"
	approvalcontract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

const approvalProposalBindingDomain = "forgeos.authenticated-architecture-decision-approval.proposal-binding.v1\x00"

func (f *authorityFixture) retargetProposal(t *testing.T, adrID, documentName string,
	supersedes []string) {
	t.Helper()
	frontmatter, body := splitTestADR(t, f.proposal)
	oldID := frontmatter["adr_id"].(string)
	body = bytes.Replace(body, []byte(oldID), []byte(adrID), 1)
	frontmatter["adr_id"], frontmatter["document_name"] = adrID, documentName
	frontmatter["supersedes"] = stringsToAny(supersedes)
	frontmatter["body_sha256"] = adrDigest("forgeos.architecture-decision-record-body.v2", body)
	frontmatter["self_sha256"] = ""
	blank := canonicalForTest(t, frontmatter)
	frontmatter["self_sha256"] = adrDigest("forgeos.architecture-decision-record.v2", blank, body)
	front := canonicalForTest(t, frontmatter)
	f.proposal = bytes.Join([][]byte{[]byte("---\n"), front, []byte("\n---\n\n"), body}, nil)
	f.rebindProposal(t, adrID, documentName, frontmatter)
	f.request["idempotency_key"] = "approval-for-" + adrID + "-0001"
	f.sealApprovalRequest(t)
}

func splitTestADR(t *testing.T, raw []byte) (map[string]any, []byte) {
	t.Helper()
	separator := []byte("\n---\n\n")
	if !bytes.HasPrefix(raw, []byte("---\n")) {
		t.Fatal("proposal opener absent")
	}
	index := bytes.Index(raw[4:], separator)
	if index < 0 {
		t.Fatal("proposal closer absent")
	}
	index += 4
	value, err := parseCanonicalJSON(raw[4:index], 64*1024, "proposal frontmatter")
	if err != nil {
		t.Fatal(err)
	}
	return value.(map[string]any), cloneBytes(raw[index+len(separator):])
}

func (f *authorityFixture) rebindProposal(t *testing.T, adrID, name string,
	frontmatter map[string]any) {
	binding := cloneObject(f.policy["proposal_binding"])
	physical := sha256.Sum256(f.proposal)
	binding["adr_id"], binding["document_name"] = adrID, name
	binding["body_sha256"], binding["self_sha256"] = frontmatter["body_sha256"], frontmatter["self_sha256"]
	binding["physical_sha256"] = hex.EncodeToString(physical[:])
	binding["proposal_binding_sha256"] = ""
	binding["proposal_binding_sha256"] = approvalSelfDigest(t, approvalProposalBindingDomain,
		binding, []string{"proposal_binding_sha256"}, false)
	f.policy["proposal_binding"] = binding
	f.request["proposal_binding"] = cloneObject(binding)
	for _, raw := range f.request["approval_records"].([]any) {
		rebindApprovalArtifacts(raw.(map[string]any), binding)
	}
	policySHA := approvalSelfDigest(t, approvalPolicyDomain, f.policy, []string{"policy_sha256"}, true)
	f.policy["policy_sha256"] = policySHA
	signTestNode(t, f.policy,
		f.private[f.byUsage["approval_policy_sign"]], approvalPolicySign, policySHA)
	f.request["policy_sha256"] = policySHA
	for _, raw := range f.request["approval_records"].([]any) {
		raw.(map[string]any)["bindings"].(map[string]any)["policy_sha256"] = policySHA
	}
	f.sealApprovalRecords(t)
	f.sealApprovalRequest(t)
}

func rebindApprovalArtifacts(record, binding map[string]any) {
	artifacts := record["bindings"].(map[string]any)["artifacts"].([]any)
	for _, raw := range artifacts {
		artifact := raw.(map[string]any)
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
	sortNodes(artifacts)
}

func adrDigest(domain string, parts ...[]byte) string {
	hasher := sha256.New()
	_, _ = hasher.Write([]byte(domain))
	for _, part := range parts {
		_, _ = hasher.Write([]byte{0})
		_, _ = hasher.Write(part)
	}
	return hex.EncodeToString(hasher.Sum(nil))
}

func (f *authorityFixture) approvalStoredIn(t *testing.T,
	stateDir string) *approvalauthority.StoredAuthorization {
	path := filepath.Join(f.approvalConfig.AuthorityRoot, stateDir)
	if err := os.Mkdir(path, privateDir); err != nil {
		t.Fatal(err)
	}
	config := f.approvalConfig
	config.StateDir = stateDir
	encoded := approvalcontract.EncodedAuthorizationInput{ProposalDocument: cloneBytes(f.proposal),
		Policy: canonicalForTest(t, f.policy), Request: canonicalForTest(t, f.request),
		RevocationSnapshots: [][]byte{canonicalForTest(t, f.revocation)}}
	trust := approvalauthority.ExternalTrust{PinnedTrustRootSHA256: f.root["root_sha256"].(string),
		PinnedTrustEpoch: 1, ObservedAtUnixMS: testObserved,
		RevocationHighWaterSequence: f.revocation["revocation_sequence"].(int64),
		RevocationHighWaterSHA256:   f.revocation["revocation_sha256"].(string)}
	stored, err := approvalauthority.AuthorizeAndStore(config, encoded, trust)
	if err != nil {
		t.Fatal(err)
	}
	return stored
}

func (f *authorityFixture) lifecycleInputAt(t *testing.T,
	stored *approvalauthority.StoredAuthorization, sequence int64,
	expectedLedger any, expectedHead string, targets []any) EncodedTransitionInput {
	source, err := stored.AcceptancePrerequisite()
	if err != nil {
		t.Fatal(err)
	}
	prerequisite, err := prerequisiteFromSource(source)
	if err != nil {
		t.Fatal(err)
	}
	request := map[string]any{"acceptance_prerequisite": prerequisite, "api_version": requestAPI,
		"canonicalization": canonicalization, "expected_current_head_set_sha256": expectedHead,
		"expected_ledger_sha256": expectedLedger, "expected_next_sequence": sequence,
		"expires_at_unix_ms": testObserved + 200_000, "idempotency_key": fmt.Sprintf("test-lifecycle-request-%04d", sequence),
		"kind": "ArchitectureDecisionLifecycleTransitionRequest", "profile_id": profileID,
		"proposal_document_base64url": base64.RawURLEncoding.EncodeToString(source.ProposalDocument),
		"request_id":                  "", "request_sha256": "", "requested_at_unix_ms": testObserved,
		"signature": signatureNode("test-lifecycle-request-key", ""), "supersession_targets": targets,
		"trust_epoch": int64(1), "trust_root_sha256": f.lifecycleRoot["root_sha256"]}
	digest, err := digestFor("request", request)
	if err != nil {
		t.Fatal(err)
	}
	request["request_sha256"] = digest
	request["request_id"] = "architecture-decision-lifecycle-request-" + digest
	request["signature"] = signatureNode("test-lifecycle-request-key", signTestDigest(t,
		f.lifecycleRequestPrivate, requestSignDomain, digest))
	return EncodedTransitionInput{RequestJSON: canonicalForTest(t, request)}
}

func statePosition(t *testing.T, state []byte) (any, string, []any) {
	t.Helper()
	node := loadRawObject(t, state)
	ledger := node["ledger"].(map[string]any)
	view := node["materialized_view"].(map[string]any)
	return ledger["ledger_sha256"].(string),
		ledger["current_head_set_sha256"].(string), view["decisions"].([]any)
}

func targetRefs(decisions []any, identifiers ...string) []any {
	byID := map[string]map[string]any{}
	for _, raw := range decisions {
		node := raw.(map[string]any)
		byID[node["adr_id"].(string)] = node
	}
	result := make([]any, len(identifiers))
	for index, identifier := range identifiers {
		node := byID[identifier]
		result[index] = map[string]any{"acceptance_id": node["acceptance_id"],
			"acceptance_sha256": node["acceptance_sha256"], "adr_id": identifier,
			"proposal_binding_sha256": node["proposal_binding_sha256"]}
	}
	return result
}
