package bootstrapgrantissuance

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
	"testing"

	"forgeos/forge-core/internal/bootstrapgrantauthority"
	"forgeos/forge-core/internal/grantstate"
)

const fixtureStoredAt = int64(1_700_000_002_000)

type fixtureDocuments struct {
	root        []byte
	policy      []byte
	request     []byte
	ledger      []byte
	result      []byte
	pin         string
	issuerSeed  []byte
	policySeed  []byte
	requestSeed []byte
}

type testLayout struct {
	config Config
	docs   fixtureDocuments
	key    string
	state  string
}

func loadFixture(t *testing.T) fixtureDocuments {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures",
		"bootstrap-grant-issuance-v1.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		t.Fatal(err)
	}
	issuerSeed, policySeed, requestSeed := randomSeed(t), randomSeed(t), randomSeed(t)
	root := rewriteTestRoot(t, raw["trust_root"], issuerSeed, policySeed, requestSeed)
	rootHash := decodeDocument(t, root)["root_sha256"].(string)
	policy := rewriteSigned(t, raw["policy"], "policy_sha256",
		[]byte("forgeos.bootstrap-grant-policy.v1\x00"),
		[]byte("forgeos.bootstrap-grant-policy.signature.v1\x00"), policySeed,
		func(document map[string]any) { document["trust_root_sha256"] = rootHash })
	policyHash := decodeDocument(t, policy)["policy_sha256"].(string)
	request := rewriteSigned(t, raw["request"], "request_sha256",
		[]byte("forgeos.bootstrap-grant-request.v1\x00"),
		[]byte("forgeos.bootstrap-grant-request.signature.v1\x00"), requestSeed,
		func(document map[string]any) {
			document["trust_root_sha256"] = rootHash
			document["policy_sha256"] = policyHash
		})
	ledger, result := buildExpectedDecision(t, root, policy, request, rootHash, issuerSeed)
	return fixtureDocuments{root: root, policy: policy, request: request,
		ledger: ledger, result: result, pin: rootHash, issuerSeed: issuerSeed,
		policySeed: policySeed, requestSeed: requestSeed}
}

func newIssuanceLayout(t *testing.T) testLayout {
	t.Helper()
	docs := loadFixture(t)
	base := t.TempDir()
	authority := filepath.Join(base, "authority")
	repository := filepath.Join(base, "repository")
	state := filepath.Join(authority, "state")
	for _, item := range []struct {
		path string
		mode os.FileMode
	}{{authority, 0o700}, {repository, 0o755}, {state, 0o700}} {
		if err := os.Mkdir(item.path, item.mode); err != nil {
			t.Fatal(err)
		}
	}
	config := Config{
		RepositoryRoot: repository, AuthorityRoot: authority, StateDir: "state",
		TrustRootPath: "trust-root.json", PolicyPath: "policy.json",
		RequestPath: "request.json", IssuerSeedPath: "issuer.seed",
		PinnedRootSHA256: docs.pin,
	}
	writeLeaf(t, authority, config.TrustRootPath, docs.root)
	writeLeaf(t, authority, config.PolicyPath, docs.policy)
	writeLeaf(t, authority, config.RequestPath, docs.request)
	key := filepath.Join(authority, config.IssuerSeedPath)
	writeMode(t, key, docs.issuerSeed, 0o600)
	return testLayout{config: config, docs: docs, key: key, state: state}
}

func writeLeaf(t *testing.T, authority, relative string, data []byte) {
	t.Helper()
	writeMode(t, filepath.Join(authority, filepath.FromSlash(relative)), data, 0o600)
}

func writeMode(t *testing.T, path string, data []byte, mode os.FileMode) {
	t.Helper()
	if err := os.WriteFile(path, data, mode); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(path, mode); err != nil {
		t.Fatal(err)
	}
}

func rewriteSigned(t *testing.T, data []byte, digestField string, digestDomain,
	signatureDomain, seed []byte, mutate func(map[string]any)) []byte {
	t.Helper()
	document := decodeDocument(t, data)
	mutate(document)
	document[digestField] = ""
	signature := document["signature"].(map[string]any)
	signature["signature_base64url"] = ""
	preimage, err := json.Marshal(document)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(append(append([]byte(nil), digestDomain...), preimage...))
	document[digestField] = hex.EncodeToString(digest[:])
	message := append(append([]byte(nil), signatureDomain...), digest[:]...)
	private := ed25519.NewKeyFromSeed(seed)
	signature["signature_base64url"] = base64.RawURLEncoding.EncodeToString(ed25519.Sign(private, message))
	clear(private)
	encoded, err := json.Marshal(document)
	if err != nil {
		t.Fatal(err)
	}
	return encoded
}

func randomSeed(t *testing.T) []byte {
	t.Helper()
	seed := make([]byte, ed25519.SeedSize)
	if _, err := rand.Read(seed); err != nil {
		t.Fatal(err)
	}
	return seed
}

func rewriteTestRoot(t *testing.T, data, issuerSeed, policySeed, requestSeed []byte) []byte {
	t.Helper()
	document := decodeDocument(t, data)
	seeds := map[string][]byte{"grant_issue": issuerSeed,
		"policy_sign": policySeed, "request_auth": requestSeed}
	for _, value := range document["keys"].([]any) {
		key := value.(map[string]any)
		private := ed25519.NewKeyFromSeed(seeds[key["usage"].(string)])
		public := private.Public().(ed25519.PublicKey)
		key["public_key_base64url"] = base64.RawURLEncoding.EncodeToString(public)
		clear(private)
	}
	document["root_sha256"] = ""
	preimage, err := json.Marshal(document)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(append([]byte("forgeos.governance-trust-root.v1\x00"), preimage...))
	document["root_sha256"] = hex.EncodeToString(digest[:])
	encoded, err := json.Marshal(document)
	if err != nil {
		t.Fatal(err)
	}
	return encoded
}

func buildExpectedDecision(t *testing.T, root, policy, request []byte, pin string,
	issuerSeed []byte) ([]byte, []byte) {
	t.Helper()
	trust, err := bootstrapgrantauthority.DecodePinnedTrustRoot(root, pin)
	if err != nil {
		t.Fatal(err)
	}
	policyValue, err := bootstrapgrantauthority.DecodePolicy(policy, trust)
	if err != nil {
		t.Fatal(err)
	}
	requestValue, err := bootstrapgrantauthority.DecodeRequest(request, trust, policyValue)
	if err != nil {
		t.Fatal(err)
	}
	issuer, err := bootstrapgrantauthority.NewIssuer(issuerSeed, trust)
	if err != nil {
		t.Fatal(err)
	}
	defer issuer.Close()
	grant, err := bootstrapgrantauthority.IssueGrant(
		policyValue, requestValue, fixtureStoredAt, issuer)
	if err != nil {
		t.Fatal(err)
	}
	receipt, err := bootstrapgrantauthority.IssueReceipt(
		policyValue, requestValue, grant, 1, nil, fixtureStoredAt, issuer)
	if err != nil {
		t.Fatal(err)
	}
	ledger, err := bootstrapgrantauthority.AppendLedger(
		nil, policyValue, requestValue, grant, receipt, issuer)
	if err != nil {
		t.Fatal(err)
	}
	result, err := bootstrapgrantauthority.StoredResult(grant, receipt)
	if err != nil {
		t.Fatal(err)
	}
	ledgerBytes, err := bootstrapgrantauthority.CanonicalLedgerJSON(ledger)
	if err != nil {
		t.Fatal(err)
	}
	resultBytes, err := bootstrapgrantauthority.CanonicalResultJSON(result)
	if err != nil {
		t.Fatal(err)
	}
	return ledgerBytes, resultBytes
}

func decodeDocument(t *testing.T, data []byte) map[string]any {
	t.Helper()
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	var document map[string]any
	if err := decoder.Decode(&document); err != nil {
		t.Fatal(err)
	}
	return document
}

func clone(data []byte) []byte { return append([]byte(nil), data...) }

type fixedClock struct {
	calls int
	err   error
	value int64
}

func (c *fixedClock) nowUnixMilli() (int64, error) {
	c.calls++
	return c.value, c.err
}

func realDependencies(clock clock) dependencies {
	return dependencies{clock: clock, openState: openGrantState}
}

func ledgerPath(layout testLayout) string {
	return filepath.Join(layout.state, grantstate.LedgerFile)
}
