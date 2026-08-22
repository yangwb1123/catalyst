package authenticatedadrapprovalauthority

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
	"strconv"
	"strings"
	"testing"

	"forgeos/forge-core/internal/approvalrecordcontract"
	contract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

const (
	testRootDomain               = "forgeos.authenticated-architecture-decision-approval.trust-root.v1\x00"
	testPolicyDomain             = "forgeos.authenticated-architecture-decision-approval.policy.v1\x00"
	testRevocationDomain         = "forgeos.authenticated-architecture-decision-approval.revocation-snapshot.v1\x00"
	testRequestDomain            = "forgeos.authenticated-architecture-decision-approval.authorization-request.v1\x00"
	testPolicySignDomain         = "forgeos.authenticated-architecture-decision-approval.policy.signature.v1\x00"
	testRevokeSignDomain         = "forgeos.authenticated-architecture-decision-approval.revocation-snapshot.signature.v1\x00"
	testRequestSignDomain        = "forgeos.authenticated-architecture-decision-approval.authorization-request.signature.v1\x00"
	testApprovalSignDomain       = "forgeos.authenticated-architecture-decision-approval.approval-record.signature.v1\x00"
	testApprovalSoDDomain        = "forgeos.authenticated-architecture-decision-approval.approval-record-sod.signature.v1\x00"
	testObservedAt         int64 = 1786748401000
)

type serviceFixture struct {
	root        map[string]any
	policy      map[string]any
	revocation  map[string]any
	revocations []map[string]any
	request     map[string]any
	proposal    []byte
	private     map[string]ed25519.PrivateKey
	byUsage     map[string]string
}

func newServiceFixture(t *testing.T) *serviceFixture {
	t.Helper()
	golden := testLoadObject(t, filepath.Join("..", "..", "..", "docs", "contracts",
		"fixtures", "authenticated-architecture-decision-approval-v1.json"))
	fixture := &serviceFixture{
		root:       cloneTestObject(golden["trust_root"]),
		policy:     cloneTestObject(golden["authorization_policy"]),
		revocation: cloneTestObject(golden["revocation_snapshot"]),
		request:    cloneTestObject(golden["authorization_request"]),
		private:    map[string]ed25519.PrivateKey{}, byUsage: map[string]string{},
	}
	proposalPath := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures",
		"ADR-9002-authenticated-approval-target.md")
	var err error
	fixture.proposal, err = os.ReadFile(proposalPath)
	if err != nil {
		t.Fatal(err)
	}
	fixture.replaceFixtureAuthority(t)
	fixture.seal(t)
	fixture.revocations = []map[string]any{fixture.revocation}
	fixture.requireValid(t)
	return fixture
}

func (f *serviceFixture) replaceFixtureAuthority(t *testing.T) {
	t.Helper()
	replacements := map[string]string{
		"forgeos.fixture.authenticated-adr-approval": "forgeos.test.authenticated-adr-approval",
	}
	keys := f.root["keys"].([]any)
	for _, item := range keys {
		key := item.(map[string]any)
		oldID := key["key_id"].(string)
		newID := "test-" + strings.TrimPrefix(oldID, "fixture-")
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
	f.root = replaceTestStrings(f.root, replacements).(map[string]any)
	f.policy = replaceTestStrings(f.policy, replacements).(map[string]any)
	f.revocation = replaceTestStrings(f.revocation, replacements).(map[string]any)
	f.request = replaceTestStrings(f.request, replacements).(map[string]any)
	testSortNodes(f.root["keys"].([]any))
}

func (f *serviceFixture) seal(t *testing.T) {
	t.Helper()
	f.root["root_sha256"] = testSelfDigest(t, testRootDomain, f.root,
		[]string{"root_sha256"}, false)
	rootSHA := f.root["root_sha256"].(string)
	for _, node := range []map[string]any{f.policy, f.revocation, f.request} {
		replaceTestField(node, "trust_root_sha256", rootSHA)
	}
	f.sealPolicy(t)
	f.sealRevocation(t)
	f.sealApprovals(t)
	f.sealRequest(t)
}

func (f *serviceFixture) sealPolicy(t *testing.T) {
	t.Helper()
	digest := testSelfDigest(t, testPolicyDomain, f.policy, []string{"policy_sha256"}, true)
	f.policy["policy_sha256"] = digest
	testSignNode(t, f.policy, f.private[f.byUsage["approval_policy_sign"]],
		testPolicySignDomain, digest)
	f.request["policy_sha256"] = digest
	for _, item := range f.request["approval_records"].([]any) {
		item.(map[string]any)["bindings"].(map[string]any)["policy_sha256"] = digest
	}
}

func (f *serviceFixture) sealRevocation(t *testing.T) {
	t.Helper()
	digest := testSelfDigest(t, testRevocationDomain, f.revocation,
		[]string{"revocation_sha256"}, true)
	f.revocation["revocation_sha256"] = digest
	testSignNode(t, f.revocation, f.private[f.byUsage["approval_revocation_sign"]],
		testRevokeSignDomain, digest)
	f.request["revocation_sha256"] = digest
}

func (f *serviceFixture) sealApprovals(t *testing.T) {
	t.Helper()
	for _, item := range f.request["approval_records"].([]any) {
		record := item.(map[string]any)
		record["approval_id"], record["approval_sha256"] = "", ""
		digest, err := approvalrecordcontract.ApprovalRecordSHA256(record)
		if err != nil {
			t.Fatal(err)
		}
		record["approval_sha256"] = digest
		record["approval_id"] = "approval-record-" + digest
		keyID := record["authority_proof"].(map[string]any)["key_id"].(string)
		record["authority_proof"].(map[string]any)["proof_base64url"] =
			testSignDigest(t, f.private[keyID], testApprovalSignDomain, digest)
		record["separation_of_duty"].(map[string]any)["proof_base64url"] =
			testSignDigest(t, f.private[keyID], testApprovalSoDDomain, digest)
	}
	testSortApprovals(f.request["approval_records"].([]any))
}

func (f *serviceFixture) sealRequest(t *testing.T) {
	t.Helper()
	f.request["request_id"], f.request["request_sha256"] = "", ""
	digest := testSelfDigest(t, testRequestDomain, f.request,
		[]string{"request_id", "request_sha256"}, true)
	f.request["request_sha256"] = digest
	f.request["request_id"] = "architecture-decision-approval-request-" + digest
	testSignNode(t, f.request, f.private[f.byUsage["approval_request_auth"]],
		testRequestSignDomain, digest)
}

func (f *serviceFixture) encoded(t *testing.T) contract.EncodedAuthorizationInput {
	t.Helper()
	snapshots := make([][]byte, len(f.revocations))
	for index, snapshot := range f.revocations {
		snapshots[index] = testCanonical(t, snapshot)
	}
	return contract.EncodedAuthorizationInput{ProposalDocument: append([]byte(nil), f.proposal...),
		Policy: testCanonical(t, f.policy), RevocationSnapshots: snapshots,
		Request: testCanonical(t, f.request)}
}

func (f *serviceFixture) trust() ExternalTrust {
	latest := f.revocations[len(f.revocations)-1]
	return ExternalTrust{PinnedTrustRootSHA256: f.root["root_sha256"].(string),
		PinnedTrustEpoch: 1, ObservedAtUnixMS: testObservedAt,
		RevocationHighWaterSequence: latest["revocation_sequence"].(int64),
		RevocationHighWaterSHA256:   latest["revocation_sha256"].(string)}
}

func (f *serviceFixture) config(t *testing.T) Config {
	t.Helper()
	config := stateTestConfig(t)
	testWritePrivate(t, filepath.Join(config.AuthorityRoot, config.TrustRootPath),
		testCanonical(t, f.root))
	seed := f.private[f.byUsage["approval_authorization_state_sign"]].Seed()
	testWritePrivate(t, filepath.Join(config.AuthorityRoot, config.StateSignerSeedPath), seed)
	return config
}

func (f *serviceFixture) requireValid(t *testing.T) {
	t.Helper()
	root, err := contract.DecodeCanonicalTrustRoot(testCanonical(t, f.root))
	if err != nil {
		t.Fatal(err)
	}
	input, err := contract.DecodeAuthorizationInput(f.encoded(t), root)
	if err != nil {
		t.Fatal(err)
	}
	checks, err := contract.SignatureChecks(input)
	if err != nil || verifySignatureChecks(checks) != nil {
		t.Fatalf("fresh fixture authentication failed: %v", err)
	}
}

func testLoadObject(t *testing.T, path string) map[string]any {
	t.Helper()
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	value, err := decodeTestObject(raw)
	if err != nil {
		t.Fatal(err)
	}
	return value
}

func cloneTestObject(value any) map[string]any {
	raw, _ := json.Marshal(value)
	result, _ := decodeTestObject(raw)
	return result
}

func decodeTestObject(raw []byte) (map[string]any, error) {
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	var value map[string]any
	if err := decoder.Decode(&value); err != nil {
		return nil, err
	}
	normalized, err := normalizeTestNumbers(value)
	if err != nil {
		return nil, err
	}
	return normalized.(map[string]any), nil
}

func normalizeTestNumbers(value any) (any, error) {
	switch typed := value.(type) {
	case map[string]any:
		for key, child := range typed {
			normalized, err := normalizeTestNumbers(child)
			if err != nil {
				return nil, err
			}
			typed[key] = normalized
		}
	case []any:
		for index, child := range typed {
			normalized, err := normalizeTestNumbers(child)
			if err != nil {
				return nil, err
			}
			typed[index] = normalized
		}
	case json.Number:
		return strconv.ParseInt(string(typed), 10, 64)
	}
	return value, nil
}

func replaceTestStrings(value any, replacements map[string]string) any {
	switch typed := value.(type) {
	case map[string]any:
		for key, child := range typed {
			typed[key] = replaceTestStrings(child, replacements)
		}
	case []any:
		for index, child := range typed {
			typed[index] = replaceTestStrings(child, replacements)
		}
	case string:
		if replacement, ok := replacements[typed]; ok {
			return replacement
		}
	}
	return value
}

func replaceTestField(value any, field, replacement string) {
	switch typed := value.(type) {
	case map[string]any:
		for key, child := range typed {
			if key == field {
				typed[key] = replacement
			} else {
				replaceTestField(child, field, replacement)
			}
		}
	case []any:
		for _, child := range typed {
			replaceTestField(child, field, replacement)
		}
	}
}

func testSelfDigest(t *testing.T, domain string, value map[string]any,
	blank []string, signed bool) string {
	t.Helper()
	preimage := cloneTestObject(value)
	for _, field := range blank {
		preimage[field] = ""
	}
	if signed {
		preimage["signature"].(map[string]any)["signature_base64url"] = ""
	}
	raw := testCanonical(t, preimage)
	digest := sha256.Sum256(append([]byte(domain), raw...))
	return hex.EncodeToString(digest[:])
}

func testSignNode(t *testing.T, node map[string]any, key ed25519.PrivateKey,
	domain, digest string) {
	t.Helper()
	node["signature"].(map[string]any)["signature_base64url"] =
		testSignDigest(t, key, domain, digest)
}

func testSignDigest(t *testing.T, key ed25519.PrivateKey, domain, digest string) string {
	t.Helper()
	raw, err := hex.DecodeString(digest)
	if err != nil || len(raw) != sha256.Size {
		t.Fatalf("bad digest %q", digest)
	}
	return base64.RawURLEncoding.EncodeToString(ed25519.Sign(key,
		append([]byte(domain), raw...)))
}

func testCanonical(t *testing.T, value any) []byte {
	t.Helper()
	raw, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func testSortNodes(values []any) {
	sort.Slice(values, func(left, right int) bool {
		leftRaw, _ := json.Marshal(values[left])
		rightRaw, _ := json.Marshal(values[right])
		return string(leftRaw) < string(rightRaw)
	})
}

func testSortApprovals(values []any) {
	sort.Slice(values, func(left, right int) bool {
		return values[left].(map[string]any)["approval_id"].(string) <
			values[right].(map[string]any)["approval_id"].(string)
	})
}

func testWritePrivate(t *testing.T, path string, raw []byte) {
	t.Helper()
	if err := os.WriteFile(path, raw, privateMode); err != nil {
		t.Fatal(err)
	}
}
