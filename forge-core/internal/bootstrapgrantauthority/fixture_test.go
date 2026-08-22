package bootstrapgrantauthority

import (
	"bytes"
	"crypto/sha256"
	"encoding/json"
	"errors"
	"os"
	"strings"
	"testing"
)

const fixturePath = "../../../docs/contracts/fixtures/bootstrap-grant-issuance-v1.json"

type fixtureContext struct {
	document map[string]any
	trust    *Trust
	policy   *Policy
	request  *Request
	issuer   *Issuer
}

func TestGoldenAuthenticatesAndReproducesIssuance(t *testing.T) {
	context := loadFixtureContext(t)
	defer context.issuer.Close()
	storedAt := int64(1_700_000_002_000)
	grant, err := IssueGrant(context.policy, context.request, storedAt, context.issuer)
	if err != nil {
		t.Fatal(err)
	}
	assertCanonicalEqual(t, grant.document, context.document["grant"])
	receipt, err := IssueReceipt(context.policy, context.request, grant, 1, nil, storedAt, context.issuer)
	if err != nil {
		t.Fatal(err)
	}
	assertCanonicalEqual(t, receipt.document, context.document["receipt"])
	ledger, err := AppendLedger(nil, context.policy, context.request, grant, receipt, context.issuer)
	if err != nil {
		t.Fatal(err)
	}
	assertCanonicalEqual(t, ledger.document, context.document["ledger"])
	result, err := StoredResult(grant, receipt)
	if err != nil {
		t.Fatal(err)
	}
	assertCanonicalEqual(t, result.document, context.document["result"])
}

func TestGoldenLedgerAuthenticatesAndReplaysExactly(t *testing.T) {
	context := loadFixtureContext(t)
	defer context.issuer.Close()
	ledgerBytes := mustCanonical(t, context.document["ledger"])
	ledger, err := DecodeLedger(ledgerBytes, context.trust)
	if err != nil {
		t.Fatal(err)
	}
	record, found, err := ledger.FindRecord(context.policy, context.request)
	if err != nil || !found {
		t.Fatalf("record lookup failed: found=%v err=%v", found, err)
	}
	result, err := record.Result()
	if err != nil {
		t.Fatal(err)
	}
	encoded, err := CanonicalResultJSON(result)
	if err != nil || !bytes.Contains(encoded, []byte(`"delivery_disposition":"exact_replay"`)) {
		t.Fatalf("exact replay result invalid: %s err=%v", encoded, err)
	}
	if ledger.NextSequence() != 2 || ledger.ClockHighWater() != 1_700_000_002_000 ||
		ledger.PriorReceiptSHA256() == nil {
		t.Fatal("ledger cursor helpers drifted")
	}
}

func TestPinnedRootAndSignaturesFailClosed(t *testing.T) {
	document := loadFixtureDocument(t)
	rootBytes := mustCanonical(t, document["trust_root"])
	if _, err := DecodePinnedTrustRoot(rootBytes, string(bytes.Repeat([]byte{'0'}, 64))); err == nil {
		t.Fatal("wrong external root pin was accepted")
	}
	trust, err := decodeKnownFixtureTrustForTest(rootBytes,
		document["trust_root"].(map[string]any)["root_sha256"].(string))
	if err != nil {
		t.Fatal(err)
	}
	policy := cloneNode(document["policy"]).(map[string]any)
	signature := policy["signature"].(map[string]any)
	signature["signature_base64url"] = string(bytes.Repeat([]byte{'A'}, 86))
	if _, err = DecodePolicy(mustCanonical(t, policy), trust); err == nil {
		t.Fatal("forged Policy signature was accepted")
	}
	if _, err = NewIssuer(bytes.Repeat([]byte{7}, 32), trust); err == nil {
		t.Fatal("wrong issuer seed was accepted")
	}
}

func TestKnownPublicFixtureAuthorityCannotEnterRuntime(t *testing.T) {
	document := loadFixtureDocument(t)
	root := document["trust_root"].(map[string]any)
	rootBytes := mustCanonical(t, root)
	rootHash := root["root_sha256"].(string)
	for _, value := range root["keys"].([]any) {
		key := value.(map[string]any)["public_key_base64url"].(string)
		if !isKnownFixturePublicKey(key) {
			t.Fatalf("fixture public key %q is absent from the runtime denylist", key)
		}
	}
	if _, err := DecodePinnedTrustRoot(rootBytes, rootHash); err == nil {
		t.Fatal("known fixture root was accepted by the runtime decoder")
	}
	trust, err := decodeKnownFixtureTrustForTest(rootBytes, rootHash)
	if err != nil {
		t.Fatal(err)
	}
	seed := sha256.Sum256([]byte("forgeos-adr0057-fixture-grant-issue-seed-v1"))
	if _, err = NewIssuer(seed[:], trust); err == nil {
		t.Fatal("known fixture issuer seed was accepted by the runtime constructor")
	}
	mutated := cloneNode(root).(map[string]any)
	mutated["trust_domain"] = "mutated.fixture.domain"
	digest, err := selfDigest(rootDomain, mutated, "root_sha256", maxRootBytes,
		"GovernanceTrustRoot", false)
	if err != nil {
		t.Fatal(err)
	}
	mutated["root_sha256"] = digest
	if _, err = DecodePinnedTrustRoot(mustCanonical(t, mutated), digest); err == nil {
		t.Fatal("mutated root containing a known fixture public key was accepted")
	}
}

func TestStrictCanonicalInputsAndUnsafePathsAreRejected(t *testing.T) {
	document := loadFixtureDocument(t)
	root := mustCanonical(t, document["trust_root"])
	rootHash := document["trust_root"].(map[string]any)["root_sha256"].(string)
	for _, malformed := range [][]byte{append(append([]byte(nil), root...), '\n'),
		bytes.Replace(root, []byte(`{"api_version"`), []byte(`{"api_version":"x","api_version"`), 1)} {
		if _, err := DecodePinnedTrustRoot(malformed, rootHash); err == nil {
			t.Fatal("noncanonical or duplicate-key root was accepted")
		}
	}
	for _, path := range []string{".", "/abs", "a/../b", "a//b", "a\\b", "a/*"} {
		if validateRepoPath(path) == nil {
			t.Fatalf("unsafe repository path %q was accepted", path)
		}
	}
}

func TestObjectKeysShareTheStringByteCeiling(t *testing.T) {
	oversizedKey := strings.Repeat("a", maxStringBytes+1)
	value := map[string]any{oversizedKey: nil}
	raw, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = parseStrictJSON(raw, maxRootBytes); err == nil {
		t.Fatal("wire object key above the string ceiling was accepted")
	}
	if _, err = canonicalJSON(value); err == nil {
		t.Fatal("programmatic object key above the string ceiling was accepted")
	}
}

func TestRequestFreshnessAndGrantPolicyExpiryAreEnforced(t *testing.T) {
	context := loadFixtureContext(t)
	defer context.issuer.Close()
	for _, now := range []int64{1_700_000_000_999, 1_700_000_301_000} {
		if ValidateIssuanceTime(context.policy, context.request, now) == nil {
			t.Fatalf("invalid issuance time %d was accepted", now)
		}
	}
	if ValidateIssuanceTime(context.policy, context.request, 1_700_000_002_000) != nil {
		t.Fatal("valid issuance time was rejected")
	}
}

func TestAuthenticatedIdempotencyConflictIsRejected(t *testing.T) {
	context := loadFixtureContext(t)
	defer context.issuer.Close()
	ledger, err := DecodeLedger(mustCanonical(t, context.document["ledger"]), context.trust)
	if err != nil {
		t.Fatal(err)
	}
	requestDocument := cloneNode(context.request.document).(map[string]any)
	bindings := requestDocument["bindings"].(map[string]any)
	bindings["source_revision"] = "different-signed-revision"
	resignDocument(t, requestDocument, "request_sha256", requestDomain,
		requestSignatureDomain, "forgeos-adr0057-fixture-request-auth-seed-v1", maxRequestBytes)
	request, err := DecodeRequest(mustCanonical(t, requestDocument), context.trust, context.policy)
	if err != nil {
		t.Fatal(err)
	}
	_, found, err := ledger.FindRecord(context.policy, request)
	if !found || !errors.Is(err, errIdempotencyConflict) {
		t.Fatalf("expected authenticated idempotency conflict, found=%v err=%v", found, err)
	}
}

func TestAuthenticatedDenyProducesNoGrant(t *testing.T) {
	context := loadFixtureContext(t)
	defer context.issuer.Close()
	policyDocument := cloneNode(context.policy.document).(map[string]any)
	policyDocument["disposition"] = "deny"
	resignDocument(t, policyDocument, "policy_sha256", policyDomain,
		policySignatureDomain, "forgeos-adr0057-fixture-policy-sign-seed-v1", maxPolicyBytes)
	policy, err := DecodePolicy(mustCanonical(t, policyDocument), context.trust)
	if err != nil {
		t.Fatal(err)
	}
	requestDocument := cloneNode(context.request.document).(map[string]any)
	requestDocument["policy_sha256"] = policy.document["policy_sha256"]
	resignDocument(t, requestDocument, "request_sha256", requestDomain,
		requestSignatureDomain, "forgeos-adr0057-fixture-request-auth-seed-v1", maxRequestBytes)
	request, err := DecodeRequest(mustCanonical(t, requestDocument), context.trust, policy)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = IssueGrant(policy, request, 1_700_000_002_000, context.issuer); err == nil {
		t.Fatal("deny Policy issued a Grant")
	}
	receipt, err := IssueReceipt(policy, request, nil, 1, nil, 1_700_000_002_000, context.issuer)
	if err != nil || receipt.document["decision"] != "denied" {
		t.Fatalf("authenticated denial receipt failed: %v", err)
	}
	if _, err = AppendLedger(nil, policy, request, nil, receipt, context.issuer); err != nil {
		t.Fatal(err)
	}
}

func TestSigningAPIsRejectMismatchedAuthenticatedPolicyAndRequest(t *testing.T) {
	context := loadFixtureContext(t)
	defer context.issuer.Close()
	policyDocument := cloneNode(context.policy.document).(map[string]any)
	setSingleScopePath(policyDocument, "outside-policy-a.md")
	resignDocument(t, policyDocument, "policy_sha256", policyDomain,
		policySignatureDomain, "forgeos-adr0057-fixture-policy-sign-seed-v1", maxPolicyBytes)
	policyB, err := DecodePolicy(mustCanonical(t, policyDocument), context.trust)
	if err != nil {
		t.Fatal(err)
	}
	requestDocument := cloneNode(context.request.document).(map[string]any)
	setSingleScopePath(requestDocument, "outside-policy-a.md")
	requestDocument["policy_sha256"] = policyB.document["policy_sha256"]
	resignDocument(t, requestDocument, "request_sha256", requestDomain,
		requestSignatureDomain, "forgeos-adr0057-fixture-request-auth-seed-v1", maxRequestBytes)
	requestB, err := DecodeRequest(mustCanonical(t, requestDocument), context.trust, policyB)
	if err != nil {
		t.Fatal(err)
	}
	storedAt := int64(1_700_000_002_000)
	if _, err = IssueGrant(context.policy, requestB, storedAt, context.issuer); err == nil {
		t.Fatal("mismatched authenticated Policy/Request issued a Grant")
	}
	if _, err = IssueReceipt(context.policy, requestB, nil, 1, nil, storedAt, context.issuer); err == nil {
		t.Fatal("mismatched authenticated Policy/Request issued a Receipt")
	}
}

func loadFixtureContext(t *testing.T) *fixtureContext {
	t.Helper()
	document := loadFixtureDocument(t)
	rootDocument := document["trust_root"].(map[string]any)
	trust, err := decodeKnownFixtureTrustForTest(mustCanonical(t, rootDocument),
		rootDocument["root_sha256"].(string))
	if err != nil {
		t.Fatal(err)
	}
	policy, err := DecodePolicy(mustCanonical(t, document["policy"]), trust)
	if err != nil {
		t.Fatal(err)
	}
	request, err := DecodeRequest(mustCanonical(t, document["request"]), trust, policy)
	if err != nil {
		t.Fatal(err)
	}
	seed := sha256.Sum256([]byte("forgeos-adr0057-fixture-grant-issue-seed-v1"))
	issuer, err := newKnownFixtureIssuerForTest(seed[:], trust)
	if err != nil {
		t.Fatal(err)
	}
	return &fixtureContext{document: document, trust: trust, policy: policy,
		request: request, issuer: issuer}
}

func decodeKnownFixtureTrustForTest(data []byte, pinned string) (*Trust, error) {
	profile := frozenSignatureProfile()
	profileHash := profile["profile_sha256"].(string)
	root, err := decodeCanonical(data, maxRootBytes)
	if err != nil {
		return nil, err
	}
	keys, err := validateTrustRoot(root, profileHash)
	if err != nil {
		return nil, err
	}
	rootHash := root["root_sha256"].(string)
	if !constantTimeTextEqual(rootHash, pinned) {
		return nil, errors.New("fixture root pin differs")
	}
	epoch, _ := intValue(root, "trust_epoch")
	domain, _ := stringValue(root, "trust_domain")
	return &Trust{profile: profile, root: root, profileHash: profileHash,
		rootHash: rootHash, epoch: epoch, domain: domain, keys: keys}, nil
}

func newKnownFixtureIssuerForTest(seed []byte, trust *Trust) (*Issuer, error) {
	if trust == nil {
		return nil, errors.New("fixture Trust is required")
	}
	publicKey, err := publicKeyFromSeed(seed)
	if err != nil {
		return nil, err
	}
	if !constantTimeTextEqual(publicKey, trust.keys["grant_issue"].publicKey) {
		return nil, errors.New("fixture issuer does not match Trust")
	}
	return &Issuer{seed: append([]byte(nil), seed...), trust: trust}, nil
}

func loadFixtureDocument(t *testing.T) map[string]any {
	t.Helper()
	raw, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatal(err)
	}
	raw = bytes.TrimSuffix(raw, []byte{'\n'})
	document, err := decodeCanonical(raw, maxGoldenBytes)
	if err != nil {
		t.Fatal(err)
	}
	return document
}

func resignDocument(t *testing.T, document map[string]any, field string, digestDomain,
	signatureDomain []byte, seedLabel string, maximum int) {
	t.Helper()
	digest, err := selfDigest(digestDomain, document, field, maximum, field, true)
	if err != nil {
		t.Fatal(err)
	}
	document[field] = digest
	seed := sha256.Sum256([]byte(seedLabel))
	signature, err := signDigest(seed[:], signatureDomain, digest)
	if err != nil {
		t.Fatal(err)
	}
	document["signature"].(map[string]any)["signature_base64url"] = signature
}

func setSingleScopePath(document map[string]any, path string) {
	scope := document["scope"].(map[string]any)
	allow := scope["allow"].([]any)
	clause := allow[0].(map[string]any)
	clause["resources"] = []any{map[string]any{
		"match": "exact", "path": path, "scope_kind": "repo_path",
	}}
}

func mustCanonical(t *testing.T, value any) []byte {
	t.Helper()
	encoded, err := canonicalJSON(value)
	if err != nil {
		t.Fatal(err)
	}
	return encoded
}

func assertCanonicalEqual(t *testing.T, left, right any) {
	t.Helper()
	if !bytes.Equal(mustCanonical(t, left), mustCanonical(t, right)) {
		t.Fatal("canonical documents differ")
	}
}
