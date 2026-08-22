package bootstraprepoexecutionauthority

import (
	"crypto/ed25519"
	"crypto/rand"
	"encoding/base64"
	"testing"

	"forgeos/forge-core/internal/bootstrapgrantauthority"
)

type runtimeContext struct {
	issuanceTrust   *bootstrapgrantauthority.Trust
	issuanceLedger  *bootstrapgrantauthority.Ledger
	trust           *Trust
	manifest        *Manifest
	policy          *Policy
	invocation      *Invocation
	signer          *Signer
	policyBytes     []byte
	invocationBytes []byte
}

func newRuntimeContext(t *testing.T) *runtimeContext {
	t.Helper()
	fixture := loadFixtureDocumentOnly(t)
	issuanceTrust, issuanceLedger := buildRuntimeIssuance(t, fixture)
	manifest, err := DecodeManifest(mustCanonicalNode(t, fixture["expected_manifest"]))
	if err != nil {
		t.Fatal(err)
	}
	trust, executionSeeds := buildRuntimeExecutionRoot(t, fixture, issuanceTrust)
	policyBytes, invocationBytes := buildRuntimeExecutionInputs(t, fixture,
		issuanceLedger, trust, executionSeeds[0], executionSeeds[2])
	policy, err := DecodePolicy(policyBytes, trust, issuanceLedger, manifest)
	if err != nil {
		t.Fatal(err)
	}
	invocation, err := DecodeInvocation(invocationBytes, trust, manifest, policy)
	if err != nil {
		t.Fatal(err)
	}
	signer, err := NewSigner(executionSeeds[1], trust)
	if err != nil {
		t.Fatal(err)
	}
	return &runtimeContext{issuanceTrust: issuanceTrust, issuanceLedger: issuanceLedger,
		trust: trust, manifest: manifest, policy: policy, invocation: invocation,
		signer: signer, policyBytes: policyBytes, invocationBytes: invocationBytes}
}

func newRuntimePair(t *testing.T) (*runtimeContext, *runtimeContext) {
	t.Helper()
	fixture := loadFixtureDocumentOnly(t)
	issuanceTrust, issuanceLedger := buildRuntimeIssuancePair(t, fixture)
	manifest, err := DecodeManifest(mustCanonicalNode(t, fixture["expected_manifest"]))
	if err != nil {
		t.Fatal(err)
	}
	trust, seeds := buildRuntimeExecutionRoot(t, fixture, issuanceTrust)
	firstPolicy, firstInvocation := buildRuntimeExecutionInputsAt(t, fixture,
		issuanceLedger, trust, seeds[0], seeds[2], 0, "")
	secondPolicy, secondInvocation := buildRuntimeExecutionInputsAt(t, fixture,
		issuanceLedger, trust, seeds[0], seeds[2], 1, "runtime-execution-request-0002")
	signer, err := NewSigner(seeds[1], trust)
	if err != nil {
		t.Fatal(err)
	}
	first := decodeRuntimeExecutionContext(t, issuanceTrust, issuanceLedger, trust,
		manifest, signer, firstPolicy, firstInvocation)
	second := decodeRuntimeExecutionContext(t, issuanceTrust, issuanceLedger, trust,
		manifest, signer, secondPolicy, secondInvocation)
	return first, second
}

func decodeRuntimeExecutionContext(t *testing.T, issuanceTrust *bootstrapgrantauthority.Trust,
	issuanceLedger *bootstrapgrantauthority.Ledger, trust *Trust, manifest *Manifest,
	signer *Signer, policyBytes, invocationBytes []byte) *runtimeContext {
	t.Helper()
	policy, err := DecodePolicy(policyBytes, trust, issuanceLedger, manifest)
	if err != nil {
		t.Fatal(err)
	}
	invocation, err := DecodeInvocation(invocationBytes, trust, manifest, policy)
	if err != nil {
		t.Fatal(err)
	}
	return &runtimeContext{issuanceTrust: issuanceTrust, issuanceLedger: issuanceLedger,
		trust: trust, manifest: manifest, policy: policy, invocation: invocation,
		signer: signer, policyBytes: policyBytes, invocationBytes: invocationBytes}
}

func buildRuntimeIssuance(t *testing.T, fixture map[string]any) (*bootstrapgrantauthority.Trust,
	*bootstrapgrantauthority.Ledger) {
	t.Helper()
	seeds, publics := generateKeys(t, 3)
	trust := buildRuntimeIssuanceTrust(t, fixture, publics)
	policy, request, _ := buildRuntimeIssuanceInputs(t, fixture, trust, seeds)
	ledger := issueRuntimeGrant(t, trust, policy, request, seeds[0])
	return trust, ledger
}

func buildRuntimeIssuanceTrust(t *testing.T, fixture map[string]any,
	publics []string) *bootstrapgrantauthority.Trust {
	t.Helper()
	root := cloneNode(fixture["issuance_trust_root"]).(map[string]any)
	for index, value := range root["keys"].([]any) {
		key := value.(map[string]any)
		key["key_id"] = []string{"test-grant-issue", "test-policy-sign", "test-request-auth"}[index]
		key["public_key_base64url"] = publics[index]
	}
	sealTestDocument(t, root, []byte("forgeos.governance-trust-root.v1\x00"),
		"root_sha256", 256*1024, false, "", nil, nil)
	trust, err := bootstrapgrantauthority.DecodePinnedTrustRoot(mustCanonicalNode(t, root),
		root["root_sha256"].(string))
	if err != nil {
		t.Fatal(err)
	}
	return trust
}

func buildRuntimeIssuanceInputs(t *testing.T, fixture map[string]any,
	trust *bootstrapgrantauthority.Trust, seeds [][]byte) (*bootstrapgrantauthority.Policy,
	*bootstrapgrantauthority.Request, string) {
	t.Helper()
	rootBinding, err := trust.ExecutionBindingJSON()
	if err != nil {
		t.Fatal(err)
	}
	binding, err := decodeCanonical(rootBinding, maxRootBytes)
	if err != nil {
		t.Fatal(err)
	}
	policyDocument := cloneNode(fixture["issuance_policy"]).(map[string]any)
	policyDocument["trust_root_sha256"], policyDocument["trust_epoch"] = binding["trust_root_sha256"], binding["trust_epoch"]
	policyDocument["signature"].(map[string]any)["key_id"] = "test-policy-sign"
	sealTestDocument(t, policyDocument, []byte("forgeos.bootstrap-grant-policy.v1\x00"),
		"policy_sha256", 512*1024, true, "", seeds[1],
		[]byte("forgeos.bootstrap-grant-policy.signature.v1\x00"))
	policy, err := bootstrapgrantauthority.DecodePolicy(mustCanonicalNode(t, policyDocument), trust)
	if err != nil {
		t.Fatal(err)
	}
	requestDocument := cloneNode(fixture["issuance_request"]).(map[string]any)
	requestDocument["trust_root_sha256"], requestDocument["trust_epoch"] = binding["trust_root_sha256"], binding["trust_epoch"]
	requestDocument["policy_sha256"] = policyDocument["policy_sha256"]
	requestDocument["signature"].(map[string]any)["key_id"] = "test-request-auth"
	sealTestDocument(t, requestDocument, []byte("forgeos.bootstrap-grant-request.v1\x00"),
		"request_sha256", 1024*1024, true, "", seeds[2],
		[]byte("forgeos.bootstrap-grant-request.signature.v1\x00"))
	request, err := bootstrapgrantauthority.DecodeRequest(mustCanonicalNode(t, requestDocument), trust, policy)
	if err != nil {
		t.Fatal(err)
	}
	return policy, request, policyDocument["policy_sha256"].(string)
}

func buildRuntimeIssuancePair(t *testing.T, fixture map[string]any) (
	*bootstrapgrantauthority.Trust, *bootstrapgrantauthority.Ledger) {
	t.Helper()
	seeds, publics := generateKeys(t, 3)
	trust := buildRuntimeIssuanceTrust(t, fixture, publics)
	policy, first, policyHash := buildRuntimeIssuanceInputs(t, fixture, trust, seeds)
	ledger := issueRuntimeGrant(t, trust, policy, first, seeds[0])
	second := buildRuntimeIssuanceRequest(t, fixture, trust, policy, policyHash,
		seeds[2], "runtime-grant-request-0002")
	issuer, err := bootstrapgrantauthority.NewIssuer(seeds[0], trust)
	if err != nil {
		t.Fatal(err)
	}
	defer issuer.Close()
	grant, err := bootstrapgrantauthority.IssueGrant(policy, second, 1_700_000_002_001, issuer)
	if err != nil {
		t.Fatal(err)
	}
	receipt, err := bootstrapgrantauthority.IssueReceipt(policy, second, grant,
		ledger.NextSequence(), ledger.PriorReceiptSHA256(), 1_700_000_002_001, issuer)
	if err != nil {
		t.Fatal(err)
	}
	ledger, err = bootstrapgrantauthority.AppendLedger(ledger, policy, second, grant, receipt, issuer)
	if err != nil {
		t.Fatal(err)
	}
	return trust, ledger
}

func buildRuntimeIssuanceRequest(t *testing.T, fixture map[string]any,
	trust *bootstrapgrantauthority.Trust, policy *bootstrapgrantauthority.Policy,
	policyHash string, seed []byte, idempotency string) *bootstrapgrantauthority.Request {
	t.Helper()
	bindingBytes, err := trust.ExecutionBindingJSON()
	if err != nil {
		t.Fatal(err)
	}
	binding, err := decodeCanonical(bindingBytes, maxRootBytes)
	if err != nil {
		t.Fatal(err)
	}
	document := cloneNode(fixture["issuance_request"]).(map[string]any)
	document["trust_root_sha256"], document["trust_epoch"] = binding["trust_root_sha256"], binding["trust_epoch"]
	document["policy_sha256"], document["idempotency_key"] = policyHash, idempotency
	document["signature"].(map[string]any)["key_id"] = "test-request-auth"
	sealTestDocument(t, document, []byte("forgeos.bootstrap-grant-request.v1\x00"),
		"request_sha256", 1024*1024, true, "", seed,
		[]byte("forgeos.bootstrap-grant-request.signature.v1\x00"))
	request, err := bootstrapgrantauthority.DecodeRequest(mustCanonicalNode(t, document), trust, policy)
	if err != nil {
		t.Fatal(err)
	}
	return request
}

func issueRuntimeGrant(t *testing.T, trust *bootstrapgrantauthority.Trust,
	policy *bootstrapgrantauthority.Policy, request *bootstrapgrantauthority.Request,
	seed []byte) *bootstrapgrantauthority.Ledger {
	t.Helper()
	issuer, err := bootstrapgrantauthority.NewIssuer(seed, trust)
	if err != nil {
		t.Fatal(err)
	}
	defer issuer.Close()
	grant, err := bootstrapgrantauthority.IssueGrant(policy, request, 1_700_000_002_000, issuer)
	if err != nil {
		t.Fatal(err)
	}
	receipt, err := bootstrapgrantauthority.IssueReceipt(policy, request, grant, 1, nil,
		1_700_000_002_000, issuer)
	if err != nil {
		t.Fatal(err)
	}
	ledger, err := bootstrapgrantauthority.AppendLedger(nil, policy, request, grant, receipt, issuer)
	if err != nil {
		t.Fatal(err)
	}
	return ledger
}

func buildRuntimeExecutionRoot(t *testing.T, fixture map[string]any,
	issuanceTrust *bootstrapgrantauthority.Trust) (*Trust, [][]byte) {
	t.Helper()
	seeds, publics := generateKeys(t, 3)
	root := cloneNode(fixture["execution_trust_root"]).(map[string]any)
	issuanceBindingBytes, err := issuanceTrust.ExecutionBindingJSON()
	if err != nil {
		t.Fatal(err)
	}
	issuanceBinding, err := decodeCanonical(issuanceBindingBytes, maxRootBytes)
	if err != nil {
		t.Fatal(err)
	}
	root["issuance_trust_root_sha256"] = issuanceBinding["trust_root_sha256"]
	root["issuance_trust_epoch"] = issuanceBinding["trust_epoch"]
	for index, value := range root["keys"].([]any) {
		key := value.(map[string]any)
		key["key_id"] = []string{"test-execution-policy", "test-execution-receipt", "test-execution-request"}[index]
		key["public_key_base64url"] = publics[index]
	}
	sealTestDocument(t, root, rootDomain, "root_sha256", maxRootBytes, false, "", nil, nil)
	trust, err := DecodePinnedTrustRoot(mustCanonicalNode(t, root), root["root_sha256"].(string), issuanceTrust)
	if err != nil {
		t.Fatal(err)
	}
	return trust, seeds
}

func buildRuntimeExecutionInputs(t *testing.T, fixture map[string]any,
	issuance *bootstrapgrantauthority.Ledger, trust *Trust,
	policySeed, requestSeed []byte) ([]byte, []byte) {
	t.Helper()
	return buildRuntimeExecutionInputsAt(t, fixture, issuance, trust, policySeed,
		requestSeed, 0, "")
}

func buildRuntimeExecutionInputsAt(t *testing.T, fixture map[string]any,
	issuance *bootstrapgrantauthority.Ledger, trust *Trust, policySeed, requestSeed []byte,
	entryIndex int, idempotency string) ([]byte, []byte) {
	t.Helper()
	ledgerBytes, err := bootstrapgrantauthority.CanonicalLedgerJSON(issuance)
	if err != nil {
		t.Fatal(err)
	}
	ledgerDocument, err := decodeCanonical(ledgerBytes, maxLedgerBytes)
	if err != nil {
		t.Fatal(err)
	}
	entry := ledgerDocument["entries"].([]any)[entryIndex].(map[string]any)
	grant := entry["grant"].(map[string]any)
	receipt := entry["receipt"].(map[string]any)
	policy := cloneNode(fixture["execution_policy"]).(map[string]any)
	if idempotency != "" {
		policy["execution_policy_id"] = "runtime-bootstrap-repo-read-policy-v2"
		policy["idempotency_key"] = idempotency
	}
	applyIssuedFields(policy, grant, receipt, trust)
	policy["signature"].(map[string]any)["key_id"] = trust.keys["execution_policy_sign"].id
	sealTestDocument(t, policy, policyDomain, "execution_policy_sha256", maxPolicyBytes,
		true, "", policySeed, policySignatureDomain)
	invocation := cloneNode(fixture["invocation"]).(map[string]any)
	for _, field := range []string{"bindings", "capability", "execution_trust_epoch",
		"execution_trust_root_sha256", "grant_envelope_sha256", "grant_id",
		"grant_issuance_ledger_sequence", "grant_issuance_receipt_sha256", "grant_policy_sha256",
		"grant_request_sha256", "grant_sha256", "idempotency_key", "issuance_trust_epoch",
		"issuance_trust_root_sha256", "manifest_sha256", "profile_id", "requested_action",
		"requested_action_sha256", "subject", "task_binding"} {
		invocation[field] = cloneNode(policy[field])
	}
	invocation["execution_policy_sha256"] = policy["execution_policy_sha256"]
	invocation["signature"].(map[string]any)["key_id"] = trust.keys["execution_request_auth"].id
	sealTestDocument(t, invocation, invocationDomain, "invocation_sha256", maxInvocationBytes,
		true, "invocation_id", requestSeed, invocationSignatureDomain)
	return mustCanonicalNode(t, policy), mustCanonicalNode(t, invocation)
}

func applyIssuedFields(policy, grant, receipt map[string]any, trust *Trust) {
	bindings := grant["bindings"].(map[string]any)
	policy["bindings"] = map[string]any{"context_sha256": bindings["context_sha256"],
		"source_revision": bindings["source_revision"], "source_tree_sha256": bindings["source_tree_sha256"]}
	for _, field := range []string{"budget", "capability", "subject", "task_binding"} {
		policy[field] = cloneNode(grant[field])
	}
	policy["execution_trust_epoch"], policy["execution_trust_root_sha256"] = trust.epoch, trust.rootHash
	policy["grant_envelope_sha256"], policy["grant_id"] = receipt["grant_envelope_sha256"], grant["grant_id"]
	policy["grant_issuance_ledger_sequence"] = receipt["ledger_sequence"]
	policy["grant_issuance_receipt_sha256"] = receipt["receipt_sha256"]
	policy["grant_policy_sha256"], policy["grant_request_sha256"] = bindings["policy_sha256"], bindings["grant_request_sha256"]
	policy["grant_sha256"] = grant["grant_sha256"]
	policy["issuance_trust_epoch"], policy["issuance_trust_root_sha256"] = trust.issuanceEpoch, trust.issuanceRootHash
}

func generateKeys(t *testing.T, count int) ([][]byte, []string) {
	t.Helper()
	seeds, publics := make([][]byte, count), make([]string, count)
	for index := 0; index < count; index++ {
		public, private, err := ed25519.GenerateKey(rand.Reader)
		if err != nil {
			t.Fatal(err)
		}
		seeds[index] = append([]byte(nil), private.Seed()...)
		publics[index] = base64.RawURLEncoding.EncodeToString(public)
	}
	return seeds, publics
}

func sealTestDocument(t *testing.T, document map[string]any, domain []byte, field string,
	maximum int, signed bool, idField string, seed, signatureDomain []byte) {
	t.Helper()
	digest, err := selfDigest(domain, document, field, maximum, "test document", signed, idField)
	if err != nil {
		t.Fatal(err)
	}
	document[field] = digest
	if idField != "" {
		document[idField] = "bootstrap-repo-read-invocation-" + digest
	}
	if signed {
		signature := document["signature"].(map[string]any)
		signature["signature_base64url"], err = signDigest(seed, signatureDomain, digest)
		if err != nil {
			t.Fatal(err)
		}
	}
}

func loadFixtureDocumentOnly(t *testing.T) map[string]any {
	t.Helper()
	return loadExecutionFixture(t).document
}
