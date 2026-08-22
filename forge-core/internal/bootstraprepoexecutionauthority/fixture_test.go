package bootstraprepoexecutionauthority

import (
	"bytes"
	"crypto/sha256"
	"encoding/base64"
	"fmt"
	"os"
	"testing"
)

const executionFixturePath = "../../../docs/contracts/fixtures/bootstrap-repo-read-execution-v1.json"

type fixtureContext struct {
	document map[string]any
	trust    *Trust
	manifest *Manifest
	ledger   *Ledger
	group    *usageGroup
}

func TestGoldenSelfContainedLedgerAndReplay(t *testing.T) {
	context := loadExecutionFixture(t)
	if context.ledger.document["ledger_sha256"] !=
		"4bc0259d280e25c703ba26fc8b5156949eb1aab136960d14289d9c8d088e1c74" {
		t.Fatal("golden UsageLedger digest drifted")
	}
	policyBytes := mustCanonicalNode(t, context.document["execution_policy"])
	invocationBytes := mustCanonicalNode(t, context.document["invocation"])
	delivery, state, found, conflict, err := context.ledger.Replay(policyBytes, invocationBytes)
	if err != nil || !found || conflict || state != "completed" || delivery == nil {
		t.Fatalf("golden replay failed: state=%s found=%v conflict=%v err=%v", state, found, conflict, err)
	}
	expected := cloneNode(context.document["first_delivery"]).(map[string]any)
	expected["delivery_disposition"] = "exact_replay"
	expected["execution_result"] = nil
	actual, err := CanonicalJSON(delivery)
	if err != nil || !bytes.Equal(actual, mustCanonicalNode(t, expected)) {
		t.Fatalf("content-free replay differs: %s err=%v", actual, err)
	}
	policyDigest := context.document["execution_policy"].(map[string]any)["execution_policy_sha256"].(string)
	invocationDigest := context.document["invocation"].(map[string]any)["invocation_sha256"].(string)
	byDigest, digestState, digestFound, digestConflict, digestErr := context.ledger.Replay(
		[]byte(policyDigest), []byte(invocationDigest))
	digestBytes, encodeErr := CanonicalJSON(byDigest)
	if digestErr != nil || encodeErr != nil || !digestFound || digestConflict ||
		digestState != state || !bytes.Equal(digestBytes, actual) {
		t.Fatalf("digest-only replay drifted: state=%s found=%v conflict=%v err=%v/%v",
			digestState, digestFound, digestConflict, digestErr, encodeErr)
	}
}

func TestGoldenRawResultMetadataAndFirstDelivery(t *testing.T) {
	context := loadExecutionFixture(t)
	resultBytes := mustCanonicalNode(t, context.document["execution_result"])
	result, err := decodeResult(resultBytes, context.group.policy,
		context.group.invocation, context.group.manifest)
	if err != nil {
		t.Fatal(err)
	}
	metadata, err := decodeMetadata(mustCanonicalNode(t, context.document["result_metadata"]), result)
	if err != nil {
		t.Fatal(err)
	}
	delivery, err := BuildDelivery("first_delivery", result, context.group.terminal.receipt, metadata)
	if err != nil {
		t.Fatal(err)
	}
	actual, _ := CanonicalJSON(delivery)
	if !bytes.Equal(actual, mustCanonicalNode(t, context.document["first_delivery"])) {
		t.Fatal("golden first delivery differs")
	}
	contents := fixtureContents(t, context.document["execution_result"])
	rebuilt, err := BuildResult(context.group.policy, context.group.invocation,
		context.group.manifest, contents, 1_700_000_007_000, 17)
	if err != nil {
		t.Fatal(err)
	}
	rebuiltBytes, _ := CanonicalJSON(rebuilt)
	if !bytes.Equal(rebuiltBytes, resultBytes) {
		t.Fatalf("rebuilt golden Result differs: %s", rebuiltBytes)
	}
	rebuiltMetadata, err := BuildMetadata(rebuilt)
	if err != nil {
		t.Fatal(err)
	}
	rebuiltMetadataBytes, _ := CanonicalJSON(rebuiltMetadata)
	if !bytes.Equal(rebuiltMetadataBytes, mustCanonicalNode(t, context.document["result_metadata"])) {
		t.Fatal("rebuilt golden metadata differs")
	}
}

func TestGoldenReceiptAndLedgerReproduce(t *testing.T) {
	context := loadExecutionFixture(t)
	seed := sha256.Sum256([]byte("forgeos-adr0058-fixture-execution-receipt-sign-seed-v1"))
	signer := &Signer{seed: append([]byte(nil), seed[:]...), trust: context.trust}
	defer signer.Close()
	var ledger *Ledger
	states := []struct {
		state, reason string
		at            int64
		metadata      *Metadata
	}{
		{"reserved_no_repo_io", "", 1_700_000_005_000, nil},
		{"effect_intent", "", 1_700_000_006_000, nil},
		{"completed", "", 1_700_000_007_000, context.group.terminal.metadata},
	}
	for index, step := range states {
		receipt, err := IssueReceipt(ledger, step.state, context.group.policy,
			context.group.invocation, context.group.manifest, step.metadata, step.at, step.reason, signer)
		if err != nil {
			t.Fatalf("issue step %d: %v", index, err)
		}
		expectedKey := []string{"reserved_receipt", "effect_intent_receipt", "completed_receipt"}[index]
		actual, _ := CanonicalJSON(receipt)
		if !bytes.Equal(actual, mustCanonicalNode(t, context.document[expectedKey])) {
			t.Fatalf("receipt step %d differs: %s", index, actual)
		}
		ledger = appendWithoutIssuanceForFixture(t, ledger, context, receipt, step.metadata, signer)
	}
	actual, _ := CanonicalJSON(ledger)
	if !bytes.Equal(actual, mustCanonicalNode(t, context.document["usage_ledger"])) {
		t.Fatal("rebuilt golden UsageLedger differs")
	}
}

func appendWithoutIssuanceForFixture(t *testing.T, current *Ledger, context *fixtureContext,
	receipt *Receipt, metadata *Metadata, signer *Signer) *Ledger {
	t.Helper()
	document, err := appendLedgerDocument(current, context.group.policy, context.group.invocation,
		context.group.manifest, receipt, metadata, signer.trust)
	if err != nil {
		t.Fatal(err)
	}
	digest, err := selfDigest(ledgerDomain, document, "ledger_sha256", maxLedgerBytes,
		"BootstrapRepoReadUsageLedger", true, "")
	if err != nil {
		t.Fatal(err)
	}
	document["ledger_sha256"] = digest
	signature := document["signature"].(map[string]any)
	signature["signature_base64url"], err = signer.sign(ledgerSignatureDomain, digest)
	if err != nil {
		t.Fatal(err)
	}
	ledger, err := validateLedgerDocument(document, signer.trust)
	if err != nil {
		t.Fatal(err)
	}
	return ledger
}

func loadExecutionFixture(t *testing.T) *fixtureContext {
	t.Helper()
	data, err := os.ReadFile(executionFixturePath)
	if err != nil {
		t.Fatal(err)
	}
	document, err := decodeCanonical(bytes.TrimSuffix(data, []byte("\n")), 24*1024*1024)
	if err != nil {
		t.Fatal(err)
	}
	issuance := fixtureIssuanceBinding(t, document["issuance_trust_root"])
	executionRoot := document["execution_trust_root"].(map[string]any)
	trust, err := decodeFixtureRootForTest(mustCanonicalNode(t, executionRoot),
		executionRoot["root_sha256"].(string), issuance)
	if err != nil {
		t.Fatal(err)
	}
	manifest, err := DecodeManifest(mustCanonicalNode(t, document["expected_manifest"]))
	if err != nil {
		t.Fatal(err)
	}
	ledgerDocument, err := decodeCanonical(mustCanonicalNode(t, document["usage_ledger"]),
		maxLedgerBytes)
	if err != nil {
		t.Fatal(err)
	}
	ledger, err := validateLedgerDocument(ledgerDocument, trust)
	if err != nil {
		t.Fatal(err)
	}
	var group *usageGroup
	for _, value := range ledger.byGrant {
		group = value
	}
	group.policy.grant = fixtureIssuedGrant(document)
	return &fixtureContext{document: document, trust: trust, manifest: manifest,
		ledger: ledger, group: group}
}

func decodeFixtureRootForTest(data []byte, pin string,
	issuance map[string]any) (*Trust, error) {
	profileHash := frozenSignatureProfile()["profile_sha256"].(string)
	root, err := decodeCanonical(data, maxRootBytes)
	if err != nil {
		return nil, err
	}
	keys, err := validateTrustRoot(root, profileHash, issuance)
	if err != nil || root["root_sha256"] != pin {
		return nil, fmt.Errorf("fixture execution TrustRoot is invalid")
	}
	epoch, _ := intValue(root, "trust_epoch")
	issuanceEpoch, _ := intValue(root, "issuance_trust_epoch")
	domain, _ := stringValue(root, "trust_domain")
	return &Trust{profileHash: profileHash, rootHash: pin, domain: domain,
		epoch: epoch, issuanceEpoch: issuanceEpoch,
		issuanceRootHash: root["issuance_trust_root_sha256"].(string), keys: keys}, nil
}

func fixtureIssuedGrant(document map[string]any) *issuedGrant {
	grant := document["grant"].(map[string]any)
	receipt := document["grant_issuance_receipt"].(map[string]any)
	bindings := grant["bindings"].(map[string]any)
	scope := grant["scope"].(map[string]any)
	allow := scope["allow"].([]any)[0].(map[string]any)
	return &issuedGrant{map[string]any{
		"bindings": map[string]any{"context_sha256": bindings["context_sha256"],
			"source_revision": bindings["source_revision"], "source_tree_sha256": bindings["source_tree_sha256"]},
		"budget": grant["budget"], "capability": grant["capability"],
		"grant_envelope_sha256": receipt["grant_envelope_sha256"], "grant_id": grant["grant_id"],
		"grant_issuance_ledger_sequence": receipt["ledger_sequence"],
		"grant_issuance_receipt_sha256":  receipt["receipt_sha256"],
		"grant_policy_sha256":            bindings["policy_sha256"], "grant_request_sha256": bindings["grant_request_sha256"],
		"grant_sha256": grant["grant_sha256"], "issuance_trust_epoch": receipt["trust_epoch"],
		"issuance_trust_root_sha256": receipt["trust_root_sha256"], "resources": allow["resources"],
		"subject": grant["subject"], "task_binding": grant["task_binding"], "validity": grant["validity"]}}
}

func fixtureIssuanceBinding(t *testing.T, value any) map[string]any {
	t.Helper()
	root := value.(map[string]any)
	keys := make([]any, 0, 3)
	for _, value := range root["keys"].([]any) {
		key := value.(map[string]any)
		keys = append(keys, map[string]any{"public_key_base64url": key["public_key_base64url"],
			"usage": key["usage"]})
	}
	return map[string]any{"keys": keys, "trust_epoch": root["trust_epoch"],
		"trust_root_sha256": root["root_sha256"]}
}

func fixtureContents(t *testing.T, value any) [][]byte {
	t.Helper()
	result := value.(map[string]any)
	reads := result["reads"].([]any)
	contents := make([][]byte, 0, len(reads))
	for _, value := range reads {
		encoded := value.(map[string]any)["content_base64url"].(string)
		decoded, err := base64.RawURLEncoding.Strict().DecodeString(encoded)
		if err != nil {
			t.Fatal(err)
		}
		contents = append(contents, decoded)
	}
	return contents
}

func mustCanonicalNode(t *testing.T, value any) []byte {
	t.Helper()
	encoded, err := canonicalJSON(value)
	if err != nil {
		t.Fatal(err)
	}
	return encoded
}
