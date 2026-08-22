//go:build unix && !aix && !solaris

package authenticatedadrlifecycleauthority

import (
	"bytes"
	"crypto/ed25519"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"

	"forgeos/forge-core/internal/approvalrecordcontract"
	approvalauthority "forgeos/forge-core/internal/authenticatedadrapprovalauthority"
	approvalcontract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

const (
	approvalRootDomain             = "forgeos.authenticated-architecture-decision-approval.trust-root.v1\x00"
	approvalPolicyDomain           = "forgeos.authenticated-architecture-decision-approval.policy.v1\x00"
	approvalRevocationDomain       = "forgeos.authenticated-architecture-decision-approval.revocation-snapshot.v1\x00"
	approvalRequestDomain          = "forgeos.authenticated-architecture-decision-approval.authorization-request.v1\x00"
	approvalPolicySign             = "forgeos.authenticated-architecture-decision-approval.policy.signature.v1\x00"
	approvalRevocationSign         = "forgeos.authenticated-architecture-decision-approval.revocation-snapshot.signature.v1\x00"
	approvalRequestSign            = "forgeos.authenticated-architecture-decision-approval.authorization-request.signature.v1\x00"
	approvalRecordSign             = "forgeos.authenticated-architecture-decision-approval.approval-record.signature.v1\x00"
	approvalRecordSoDSign          = "forgeos.authenticated-architecture-decision-approval.approval-record-sod.signature.v1\x00"
	testObserved             int64 = 1786748401000
)

type authorityFixture struct {
	root, policy, revocation, request map[string]any
	proposal                          []byte
	private                           map[string]ed25519.PrivateKey
	byUsage                           map[string]string
	profile                           []byte
	approvalConfig                    approvalauthority.Config
	lifecycleConfig                   Config
	lifecycleRequestPrivate           ed25519.PrivateKey
	lifecycleStatePrivate             ed25519.PrivateKey
	lifecycleRoot                     map[string]any
}

func newAuthorityFixture(t *testing.T) *authorityFixture {
	t.Helper()
	golden := loadJSONObject(t, filepath.Join("..", "..", "..", "docs", "contracts",
		"fixtures", "authenticated-architecture-decision-approval-v1.json"))
	fixture := &authorityFixture{root: cloneObject(golden["trust_root"]),
		policy:     cloneObject(golden["authorization_policy"]),
		revocation: cloneObject(golden["revocation_snapshot"]),
		request:    cloneObject(golden["authorization_request"]),
		private:    map[string]ed25519.PrivateKey{}, byUsage: map[string]string{}}
	fixture.profile = canonicalForTest(t, golden["signature_profile"])
	fixture.proposal = readTestFile(t, filepath.Join("..", "..", "..", "docs", "contracts",
		"fixtures", "ADR-9002-authenticated-approval-target.md"))
	fixture.replaceApprovalKeys(t)
	fixture.sealApproval(t)
	fixture.createRoots(t)
	return fixture
}

func (f *authorityFixture) replaceApprovalKeys(t *testing.T) {
	replacements := map[string]string{"forgeos.fixture.authenticated-adr-approval": "forgeos.test.lifecycle-approval"}
	for _, raw := range f.root["keys"].([]any) {
		key := raw.(map[string]any)
		oldID := key["key_id"].(string)
		newID := "test-lifecycle-" + strings.TrimPrefix(oldID, "fixture-")
		public, private, err := ed25519.GenerateKey(rand.Reader)
		if err != nil {
			t.Fatal(err)
		}
		replacements[oldID] = newID
		key["key_id"] = newID
		key["public_key_base64url"] = base64.RawURLEncoding.EncodeToString(public)
		f.private[newID] = private
		f.byUsage[key["usage"].(string)] = newID
	}
	f.root = replaceStrings(f.root, replacements).(map[string]any)
	f.policy = replaceStrings(f.policy, replacements).(map[string]any)
	f.revocation = replaceStrings(f.revocation, replacements).(map[string]any)
	f.request = replaceStrings(f.request, replacements).(map[string]any)
	sortNodes(f.root["keys"].([]any))
}

func (f *authorityFixture) sealApproval(t *testing.T) {
	f.root["root_sha256"] = approvalSelfDigest(t, approvalRootDomain, f.root, []string{"root_sha256"}, false)
	rootSHA := f.root["root_sha256"].(string)
	for _, node := range []map[string]any{f.policy, f.revocation, f.request} {
		replaceField(node, "trust_root_sha256", rootSHA)
	}
	policySHA := approvalSelfDigest(t, approvalPolicyDomain, f.policy, []string{"policy_sha256"}, true)
	f.policy["policy_sha256"] = policySHA
	signTestNode(t, f.policy, f.private[f.byUsage["approval_policy_sign"]], approvalPolicySign, policySHA)
	f.request["policy_sha256"] = policySHA
	for _, raw := range f.request["approval_records"].([]any) {
		raw.(map[string]any)["bindings"].(map[string]any)["policy_sha256"] = policySHA
	}
	revocationSHA := approvalSelfDigest(t, approvalRevocationDomain, f.revocation, []string{"revocation_sha256"}, true)
	f.revocation["revocation_sha256"] = revocationSHA
	signTestNode(t, f.revocation, f.private[f.byUsage["approval_revocation_sign"]], approvalRevocationSign, revocationSHA)
	f.request["revocation_sha256"] = revocationSHA
	f.sealApprovalRecords(t)
	f.sealApprovalRequest(t)
}

func (f *authorityFixture) sealApprovalRecords(t *testing.T) {
	for _, raw := range f.request["approval_records"].([]any) {
		record := raw.(map[string]any)
		record["approval_id"], record["approval_sha256"] = "", ""
		digest, err := approvalrecordcontract.ApprovalRecordSHA256(record)
		if err != nil {
			t.Fatal(err)
		}
		record["approval_sha256"] = digest
		record["approval_id"] = "approval-record-" + digest
		keyID := record["authority_proof"].(map[string]any)["key_id"].(string)
		record["authority_proof"].(map[string]any)["proof_base64url"] = signTestDigest(t, f.private[keyID], approvalRecordSign, digest)
		record["separation_of_duty"].(map[string]any)["proof_base64url"] = signTestDigest(t, f.private[keyID], approvalRecordSoDSign, digest)
	}
	sort.Slice(f.request["approval_records"].([]any), func(left, right int) bool {
		items := f.request["approval_records"].([]any)
		return items[left].(map[string]any)["approval_id"].(string) < items[right].(map[string]any)["approval_id"].(string)
	})
}

func (f *authorityFixture) sealApprovalRequest(t *testing.T) {
	f.request["request_id"], f.request["request_sha256"] = "", ""
	digest := approvalSelfDigest(t, approvalRequestDomain, f.request, []string{"request_id", "request_sha256"}, true)
	f.request["request_sha256"] = digest
	f.request["request_id"] = "architecture-decision-approval-request-" + digest
	signTestNode(t, f.request, f.private[f.byUsage["approval_request_auth"]], approvalRequestSign, digest)
}

func (f *authorityFixture) createRoots(t *testing.T) {
	base := t.TempDir()
	repository := filepath.Join(base, "repository")
	authority := filepath.Join(base, "authority")
	for _, path := range []string{repository, authority, filepath.Join(authority, "approval-state"), filepath.Join(authority, "lifecycle-state")} {
		if err := os.Mkdir(path, privateDir); err != nil {
			t.Fatal(err)
		}
	}
	writePrivateTest(t, filepath.Join(authority, "profile.json"), f.profile)
	writePrivateTest(t, filepath.Join(authority, "approval-root.json"), canonicalForTest(t, f.root))
	writePrivateTest(t, filepath.Join(authority, "approval-state.seed"), f.private[f.byUsage["approval_authorization_state_sign"]].Seed())
	f.lifecycleRoot, f.lifecycleRequestPrivate, f.lifecycleStatePrivate = newLifecycleRoot(t)
	writePrivateTest(t, filepath.Join(authority, "lifecycle-root.json"), canonicalForTest(t, f.lifecycleRoot))
	writePrivateTest(t, filepath.Join(authority, "lifecycle-state.seed"), f.lifecycleStatePrivate.Seed())
	writePrivateTest(t, filepath.Join(authority, "lifecycle-state", lockFile), nil)
	f.approvalConfig = approvalauthority.Config{RepositoryRoot: repository, AuthorityRoot: authority,
		StateDir: "approval-state", TrustRootPath: "approval-root.json", StateSignerSeedPath: "approval-state.seed"}
	f.lifecycleConfig = Config{RepositoryRoot: repository, AuthorityRoot: authority,
		StateDir: "lifecycle-state", SignatureProfilePath: "profile.json",
		ApprovalTrustRootPath: "approval-root.json", LifecycleTrustRootPath: "lifecycle-root.json",
		StateSignerSeedPath: "lifecycle-state.seed"}
}

func newLifecycleRoot(t *testing.T) (map[string]any, ed25519.PrivateKey, ed25519.PrivateKey) {
	requestPublic, requestPrivate, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	statePublic, statePrivate, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	principal := func(id, kind string) map[string]any {
		return map[string]any{"authority_domain": "forgeos.test.lifecycle", "principal_id": id, "principal_type": kind}
	}
	keys := []any{
		map[string]any{"key_id": "test-lifecycle-request-key", "principal": principal("requester", "operator"), "public_key_base64url": base64.RawURLEncoding.EncodeToString(requestPublic), "usage": requestUsage},
		map[string]any{"key_id": "test-lifecycle-state-key", "principal": principal("state-service", "service"), "public_key_base64url": base64.RawURLEncoding.EncodeToString(statePublic), "usage": stateUsage},
	}
	sortNodes(keys)
	root := map[string]any{"api_version": "forgeos.architecture-decision-lifecycle-trust-root/v1",
		"canonicalization": canonicalization, "keys": keys, "kind": "ArchitectureDecisionLifecycleTrustRoot",
		"profile_id": profileID, "root_sha256": "", "signature_profile_sha256": profileSHA256,
		"trust_domain": "forgeos.test.lifecycle", "trust_epoch": int64(1)}
	digest, err := digestFor("root", root)
	if err != nil {
		t.Fatal(err)
	}
	root["root_sha256"] = digest
	return root, requestPrivate, statePrivate
}

func (f *authorityFixture) approvalStored(t *testing.T) *approvalauthority.StoredAuthorization {
	encoded := approvalcontract.EncodedAuthorizationInput{ProposalDocument: cloneBytes(f.proposal),
		Policy: canonicalForTest(t, f.policy), Request: canonicalForTest(t, f.request),
		RevocationSnapshots: [][]byte{canonicalForTest(t, f.revocation)}}
	trust := approvalauthority.ExternalTrust{PinnedTrustRootSHA256: f.root["root_sha256"].(string),
		PinnedTrustEpoch: 1, ObservedAtUnixMS: testObserved,
		RevocationHighWaterSequence: f.revocation["revocation_sequence"].(int64),
		RevocationHighWaterSHA256:   f.revocation["revocation_sha256"].(string)}
	stored, err := approvalauthority.AuthorizeAndStore(f.approvalConfig, encoded, trust)
	if err != nil {
		t.Fatal(err)
	}
	return stored
}

func (f *authorityFixture) lifecycleTrust() ExternalTrust {
	return ExternalTrust{PinnedApprovalTrustRootSHA256: f.root["root_sha256"].(string),
		PinnedApprovalTrustEpoch: 1, PinnedLifecycleTrustRootSHA256: f.lifecycleRoot["root_sha256"].(string),
		PinnedLifecycleTrustEpoch: 1, ObservedAtUnixMS: testObserved}
}

func (f *authorityFixture) lifecycleInput(t *testing.T,
	stored *approvalauthority.StoredAuthorization) EncodedTransitionInput {
	source, err := stored.AcceptancePrerequisite()
	if err != nil {
		t.Fatal(err)
	}
	prerequisite, err := prerequisiteFromSource(source)
	if err != nil {
		t.Fatal(err)
	}
	head := domainDigest(headDomain, []byte("[]"))
	request := map[string]any{"acceptance_prerequisite": prerequisite, "api_version": requestAPI,
		"canonicalization": canonicalization, "expected_current_head_set_sha256": head,
		"expected_ledger_sha256": nil, "expected_next_sequence": int64(1),
		"expires_at_unix_ms": testObserved + 200_000, "idempotency_key": "test-lifecycle-request-0001",
		"kind": "ArchitectureDecisionLifecycleTransitionRequest", "profile_id": profileID,
		"proposal_document_base64url": base64.RawURLEncoding.EncodeToString(source.ProposalDocument),
		"request_id":                  "", "request_sha256": "", "requested_at_unix_ms": testObserved,
		"signature": signatureNode("test-lifecycle-request-key", ""), "supersession_targets": []any{},
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

func loadJSONObject(t *testing.T, path string) map[string]any {
	t.Helper()
	raw := readTestFile(t, path)
	raw = bytes.TrimSuffix(raw, []byte{'\n'})
	value, err := parseCanonicalJSON(raw, maxBundle, "test JSON")
	if err != nil {
		t.Fatal(err)
	}
	return value.(map[string]any)
}

func readTestFile(t *testing.T, path string) []byte {
	t.Helper()
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return raw
}
func cloneObject(value any) map[string]any { return cloneValue(value).(map[string]any) }

func replaceStrings(value any, replacements map[string]string) any {
	switch typed := value.(type) {
	case map[string]any:
		for key, child := range typed {
			typed[key] = replaceStrings(child, replacements)
		}
	case []any:
		for index, child := range typed {
			typed[index] = replaceStrings(child, replacements)
		}
	case string:
		if replacement, ok := replacements[typed]; ok {
			return replacement
		}
	}
	return value
}

func replaceField(value any, field, replacement string) {
	switch typed := value.(type) {
	case map[string]any:
		for key, child := range typed {
			if key == field {
				typed[key] = replacement
			} else {
				replaceField(child, field, replacement)
			}
		}
	case []any:
		for _, child := range typed {
			replaceField(child, field, replacement)
		}
	}
}

func approvalSelfDigest(t *testing.T, domain string, value map[string]any,
	blank []string, signed bool) string {
	t.Helper()
	preimage := cloneObject(value)
	for _, field := range blank {
		preimage[field] = ""
	}
	if signed {
		preimage["signature"].(map[string]any)["signature_base64url"] = ""
	}
	raw := canonicalForTest(t, preimage)
	digest := sha256.Sum256(append([]byte(domain), raw...))
	return hex.EncodeToString(digest[:])
}

func signTestNode(t *testing.T, node map[string]any, key ed25519.PrivateKey,
	domain, digest string) {
	node["signature"].(map[string]any)["signature_base64url"] = signTestDigest(t, key, domain, digest)
}

func signTestDigest(t *testing.T, key ed25519.PrivateKey, domain, digest string) string {
	t.Helper()
	raw, err := hex.DecodeString(digest)
	if err != nil || len(raw) != 32 {
		t.Fatal("bad digest")
	}
	return base64.RawURLEncoding.EncodeToString(ed25519.Sign(key, append([]byte(domain), raw...)))
}

func canonicalForTest(t *testing.T, value any) []byte {
	t.Helper()
	raw, err := canonicalJSON(value, maxBundle, "test value")
	if err != nil {
		t.Fatal(err)
	}
	return raw
}
func sortNodes(values []any) {
	sort.Slice(values, func(left, right int) bool {
		leftRaw, _ := json.Marshal(values[left])
		rightRaw, _ := json.Marshal(values[right])
		return string(leftRaw) < string(rightRaw)
	})
}
func writePrivateTest(t *testing.T, path string, raw []byte) {
	t.Helper()
	if err := os.WriteFile(path, raw, privateMode); err != nil {
		t.Fatal(err)
	}
}
