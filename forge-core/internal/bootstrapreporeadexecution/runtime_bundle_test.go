//go:build linux && (amd64 || arm64)

package bootstrapreporeadexecution

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"forgeos/forge-core/internal/bootstrapgrantauthority"
	"forgeos/forge-core/internal/bootstraprepoexecutionauthority"
)

const executionFixture = "../../../docs/contracts/fixtures/bootstrap-repo-read-execution-v1.json"

type runtimeBundle struct {
	executionRoot, invocation, issuanceLedger []byte
	issuanceRoot, manifest, policy, seed      []byte
	executionPin, issuancePin                 string
	fixture                                   map[string]any
}

type issuanceBundle struct {
	ledger     *bootstrapgrantauthority.Ledger
	ledgerData []byte
	root       map[string]any
	rootData   []byte
	trust      *bootstrapgrantauthority.Trust
}

type executionBundle struct {
	policySeed, receiptSeed, requestSeed []byte
	root                                 map[string]any
	rootData                             []byte
	trust                                *bootstraprepoexecutionauthority.Trust
}

func newRuntimeBundle(t *testing.T) runtimeBundle {
	t.Helper()
	fixture := loadRuntimeFixture(t)
	issuance := buildRuntimeIssuance(t, fixture)
	execution := buildExecutionRoot(t, fixture, issuance)
	policy, invocation := buildExecutionDocuments(t, fixture, issuance, execution)
	return runtimeBundle{executionRoot: execution.rootData, invocation: invocation,
		issuanceLedger: issuance.ledgerData, issuanceRoot: issuance.rootData,
		manifest: canonicalTestNode(t, fixture["expected_manifest"]), policy: policy,
		seed: execution.receiptSeed, fixture: fixture,
		executionPin: execution.root["root_sha256"].(string),
		issuancePin:  issuance.root["root_sha256"].(string)}
}

func buildRuntimeIssuance(t *testing.T, fixture map[string]any) issuanceBundle {
	t.Helper()
	keys := generateTestKeys(t, 3)
	root := cloneTestNode(t, fixture["issuance_trust_root"]).(map[string]any)
	for index, value := range root["keys"].([]any) {
		key := value.(map[string]any)
		key["key_id"] = []string{"runtime-grant-issue", "runtime-policy-sign",
			"runtime-request-auth"}[index]
		key["public_key_base64url"] = keys.public[index]
	}
	sealTestDocument(t, root, []byte("forgeos.governance-trust-root.v1\x00"),
		"root_sha256", false, "", nil, nil)
	rootData := canonicalTestNode(t, root)
	trust, err := bootstrapgrantauthority.DecodePinnedTrustRoot(
		rootData, root["root_sha256"].(string))
	if err != nil {
		t.Fatal(err)
	}
	ledger := buildIssuanceLedger(t, fixture, root, trust, keys)
	ledgerData, err := bootstrapgrantauthority.CanonicalLedgerJSON(ledger)
	if err != nil {
		t.Fatal(err)
	}
	return issuanceBundle{ledger: ledger, ledgerData: ledgerData,
		root: root, rootData: rootData, trust: trust}
}

func buildIssuanceLedger(t *testing.T, fixture, root map[string]any,
	trust *bootstrapgrantauthority.Trust, keys testKeys) *bootstrapgrantauthority.Ledger {
	t.Helper()
	policyDocument := cloneTestNode(t, fixture["issuance_policy"]).(map[string]any)
	policyDocument["trust_root_sha256"], policyDocument["trust_epoch"] =
		root["root_sha256"], root["trust_epoch"]
	policyDocument["signature"].(map[string]any)["key_id"] = "runtime-policy-sign"
	sealTestDocument(t, policyDocument, []byte("forgeos.bootstrap-grant-policy.v1\x00"),
		"policy_sha256", true, "", keys.seeds[1],
		[]byte("forgeos.bootstrap-grant-policy.signature.v1\x00"))
	policy, err := bootstrapgrantauthority.DecodePolicy(canonicalTestNode(t, policyDocument), trust)
	if err != nil {
		t.Fatal(err)
	}
	request := buildIssuanceRequest(t, fixture, root, policyDocument, trust, keys.seeds[2])
	issuer, err := bootstrapgrantauthority.NewIssuer(keys.seeds[0], trust)
	if err != nil {
		t.Fatal(err)
	}
	defer issuer.Close()
	grant, err := bootstrapgrantauthority.IssueGrant(policy, request, 1_700_000_002_000, issuer)
	if err != nil {
		t.Fatal(err)
	}
	receipt, err := bootstrapgrantauthority.IssueReceipt(policy, request, grant, 1,
		nil, 1_700_000_002_000, issuer)
	if err != nil {
		t.Fatal(err)
	}
	ledger, err := bootstrapgrantauthority.AppendLedger(nil, policy, request, grant, receipt, issuer)
	if err != nil {
		t.Fatal(err)
	}
	return ledger
}

func buildIssuanceRequest(t *testing.T, fixture, root, policy map[string]any,
	trust *bootstrapgrantauthority.Trust, seed []byte) *bootstrapgrantauthority.Request {
	t.Helper()
	document := cloneTestNode(t, fixture["issuance_request"]).(map[string]any)
	document["trust_root_sha256"], document["trust_epoch"] =
		root["root_sha256"], root["trust_epoch"]
	document["policy_sha256"] = policy["policy_sha256"]
	document["signature"].(map[string]any)["key_id"] = "runtime-request-auth"
	sealTestDocument(t, document, []byte("forgeos.bootstrap-grant-request.v1\x00"),
		"request_sha256", true, "", seed,
		[]byte("forgeos.bootstrap-grant-request.signature.v1\x00"))
	request, err := bootstrapgrantauthority.DecodeRequest(canonicalTestNode(t, document), trust,
		mustDecodeIssuancePolicy(t, policy, trust))
	if err != nil {
		t.Fatal(err)
	}
	return request
}

func mustDecodeIssuancePolicy(t *testing.T, document map[string]any,
	trust *bootstrapgrantauthority.Trust) *bootstrapgrantauthority.Policy {
	t.Helper()
	policy, err := bootstrapgrantauthority.DecodePolicy(canonicalTestNode(t, document), trust)
	if err != nil {
		t.Fatal(err)
	}
	return policy
}

func buildExecutionRoot(t *testing.T, fixture map[string]any,
	issuance issuanceBundle) executionBundle {
	t.Helper()
	keys := generateTestKeys(t, 3)
	root := cloneTestNode(t, fixture["execution_trust_root"]).(map[string]any)
	root["issuance_trust_root_sha256"], root["issuance_trust_epoch"] =
		issuance.root["root_sha256"], issuance.root["trust_epoch"]
	for index, value := range root["keys"].([]any) {
		key := value.(map[string]any)
		key["key_id"] = []string{"runtime-execution-policy", "runtime-execution-receipt",
			"runtime-execution-request"}[index]
		key["public_key_base64url"] = keys.public[index]
	}
	sealTestDocument(t, root,
		[]byte("forgeos.bootstrap-repo-read-execution-trust-root.v1\x00"),
		"root_sha256", false, "", nil, nil)
	data := canonicalTestNode(t, root)
	trust, err := bootstraprepoexecutionauthority.DecodePinnedTrustRoot(
		data, root["root_sha256"].(string), issuance.trust)
	if err != nil {
		t.Fatal(err)
	}
	return executionBundle{policySeed: keys.seeds[0], receiptSeed: keys.seeds[1],
		requestSeed: keys.seeds[2], root: root, rootData: data, trust: trust}
}

func buildExecutionDocuments(t *testing.T, fixture map[string]any, issuance issuanceBundle,
	execution executionBundle) ([]byte, []byte) {
	t.Helper()
	entry := decodeTestJSON(t, issuance.ledgerData)["entries"].([]any)[0].(map[string]any)
	grant, receipt := entry["grant"].(map[string]any), entry["receipt"].(map[string]any)
	policy := cloneTestNode(t, fixture["execution_policy"]).(map[string]any)
	applyIssuedPolicy(policy, grant, receipt, issuance.root, execution.root)
	policy["signature"].(map[string]any)["key_id"] = "runtime-execution-policy"
	sealTestDocument(t, policy, []byte("forgeos.bootstrap-repo-read-execution-policy.v1\x00"),
		"execution_policy_sha256", true, "", execution.policySeed,
		[]byte("forgeos.bootstrap-repo-read-execution-policy.signature.v1\x00"))
	invocation := cloneTestNode(t, fixture["invocation"]).(map[string]any)
	copyInvocationBindings(invocation, policy)
	invocation["signature"].(map[string]any)["key_id"] = "runtime-execution-request"
	sealTestDocument(t, invocation, []byte("forgeos.bootstrap-repo-read-invocation.v1\x00"),
		"invocation_sha256", true, "invocation_id", execution.requestSeed,
		[]byte("forgeos.bootstrap-repo-read-invocation.signature.v1\x00"))
	return canonicalTestNode(t, policy), canonicalTestNode(t, invocation)
}

func applyIssuedPolicy(policy, grant, receipt, issuanceRoot, executionRoot map[string]any) {
	bindings := grant["bindings"].(map[string]any)
	policy["bindings"] = map[string]any{"context_sha256": bindings["context_sha256"],
		"source_revision":    bindings["source_revision"],
		"source_tree_sha256": bindings["source_tree_sha256"]}
	for _, field := range []string{"budget", "capability", "subject", "task_binding"} {
		policy[field] = cloneNodeWithoutTest(grant[field])
	}
	policy["execution_trust_epoch"] = executionRoot["trust_epoch"]
	policy["execution_trust_root_sha256"] = executionRoot["root_sha256"]
	policy["grant_envelope_sha256"], policy["grant_id"] =
		receipt["grant_envelope_sha256"], grant["grant_id"]
	policy["grant_issuance_ledger_sequence"] = receipt["ledger_sequence"]
	policy["grant_issuance_receipt_sha256"] = receipt["receipt_sha256"]
	policy["grant_policy_sha256"], policy["grant_request_sha256"] =
		bindings["policy_sha256"], bindings["grant_request_sha256"]
	policy["grant_sha256"] = grant["grant_sha256"]
	policy["issuance_trust_epoch"] = issuanceRoot["trust_epoch"]
	policy["issuance_trust_root_sha256"] = issuanceRoot["root_sha256"]
}

func copyInvocationBindings(invocation, policy map[string]any) {
	fields := []string{"bindings", "capability", "execution_trust_epoch",
		"execution_trust_root_sha256", "grant_envelope_sha256", "grant_id",
		"grant_issuance_ledger_sequence", "grant_issuance_receipt_sha256",
		"grant_policy_sha256", "grant_request_sha256", "grant_sha256", "idempotency_key",
		"issuance_trust_epoch", "issuance_trust_root_sha256", "manifest_sha256", "profile_id",
		"requested_action", "requested_action_sha256", "subject", "task_binding"}
	for _, field := range fields {
		invocation[field] = cloneNodeWithoutTest(policy[field])
	}
	invocation["execution_policy_sha256"] = policy["execution_policy_sha256"]
}

func cloneNodeWithoutTest(value any) any {
	switch typed := value.(type) {
	case map[string]any:
		result := make(map[string]any, len(typed))
		for key, child := range typed {
			result[key] = cloneNodeWithoutTest(child)
		}
		return result
	case []any:
		result := make([]any, len(typed))
		for index, child := range typed {
			result[index] = cloneNodeWithoutTest(child)
		}
		return result
	default:
		return value
	}
}

func loadRuntimeFixture(t *testing.T) map[string]any {
	t.Helper()
	data, err := os.ReadFile(executionFixture)
	if err != nil {
		t.Fatal(err)
	}
	return decodeTestJSON(t, bytes.TrimSuffix(data, []byte("\n")))
}

func decodeTestJSON(t *testing.T, data []byte) map[string]any {
	t.Helper()
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	var value map[string]any
	if err := decoder.Decode(&value); err != nil {
		t.Fatal(err)
	}
	return value
}

func fixtureRepositoryContent(t *testing.T, fixture map[string]any) map[string][]byte {
	t.Helper()
	result := map[string][]byte{}
	reads := fixture["execution_result"].(map[string]any)["reads"].([]any)
	for _, value := range reads {
		read := value.(map[string]any)
		content, err := base64.RawURLEncoding.Strict().DecodeString(
			read["content_base64url"].(string))
		if err != nil {
			t.Fatal(err)
		}
		result[read["path"].(string)] = content
	}
	return result
}

func writeRuntimeFile(t *testing.T, root, relative string, content []byte) {
	t.Helper()
	name := filepath.Join(root, filepath.FromSlash(relative))
	if err := os.MkdirAll(filepath.Dir(name), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(name, content, 0o600); err != nil {
		t.Fatal(err)
	}
}
