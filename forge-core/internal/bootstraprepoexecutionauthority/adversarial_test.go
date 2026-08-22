package bootstraprepoexecutionauthority

import (
	"bytes"
	"crypto/sha256"
	"strings"
	"testing"
)

func TestKnownFixtureAuthorityIsProductionRejected(t *testing.T) {
	context := loadExecutionFixture(t)
	root := context.document["execution_trust_root"].(map[string]any)
	issuance := fixtureIssuanceBinding(t, context.document["issuance_trust_root"])
	if _, err := decodePinnedRoot(mustCanonicalNode(t, root), root["root_sha256"].(string),
		issuance); err == nil {
		t.Fatal("known fixture execution root entered production decoder")
	}
	seed := sha256.Sum256([]byte("forgeos-adr0058-fixture-execution-receipt-sign-seed-v1"))
	if _, err := NewSigner(seed[:], context.trust); err == nil {
		t.Fatal("known fixture signer entered production constructor")
	}
	mutated := cloneNode(root).(map[string]any)
	mutated["trust_domain"] = "mutated.fixture.execution"
	digest, err := selfDigest(rootDomain, mutated, "root_sha256", maxRootBytes,
		"BootstrapRepoReadExecutionTrustRoot", false, "")
	if err != nil {
		t.Fatal(err)
	}
	mutated["root_sha256"] = digest
	if _, err = decodePinnedRoot(mustCanonicalNode(t, mutated), digest, issuance); err == nil {
		t.Fatal("mutated root containing fixture keys entered production decoder")
	}
}

func TestStrictCanonicalAndSignatureInputsFailClosed(t *testing.T) {
	context := loadExecutionFixture(t)
	root := mustCanonicalNode(t, context.document["execution_trust_root"])
	issuance := fixtureIssuanceBinding(t, context.document["issuance_trust_root"])
	pin := context.document["execution_trust_root"].(map[string]any)["root_sha256"].(string)
	for _, malformed := range [][]byte{append(append([]byte(nil), root...), '\n'),
		bytes.Replace(root, []byte(`{"api_version"`), []byte(`{"api_version":"x","api_version"`), 1)} {
		if _, err := decodeFixtureRootForTest(malformed, pin, issuance); err == nil {
			t.Fatal("noncanonical or duplicate-key root was accepted")
		}
	}
	policy := cloneNode(context.document["execution_policy"]).(map[string]any)
	policy["signature"].(map[string]any)["signature_base64url"] = string(bytes.Repeat([]byte{'A'}, 86))
	if _, _, _, _, err := context.ledger.Replay(mustCanonicalNode(t, policy),
		mustCanonicalNode(t, context.document["invocation"])); err == nil {
		t.Fatal("forged execution Policy signature was accepted for replay")
	}
}

func TestManifestBoundsOrderingAndDigestFailClosed(t *testing.T) {
	context := loadExecutionFixture(t)
	manifest := cloneNode(context.document["expected_manifest"]).(map[string]any)
	entries := manifest["entries"].([]any)
	entries[0], entries[1] = entries[1], entries[0]
	reselfDigest(t, manifest, manifestDomain, "manifest_sha256", maxManifestBytes, false, "", nil, nil)
	if _, err := DecodeManifest(mustCanonicalNode(t, manifest)); err == nil {
		t.Fatal("unsorted Manifest paths were accepted")
	}
	manifest = cloneNode(context.document["expected_manifest"]).(map[string]any)
	manifest["entries"].([]any)[0].(map[string]any)["content_bytes"] = maxContentBytes + 1
	reselfDigest(t, manifest, manifestDomain, "manifest_sha256", maxManifestBytes, false, "", nil, nil)
	if _, err := DecodeManifest(mustCanonicalNode(t, manifest)); err == nil {
		t.Fatal("oversized Manifest entry was accepted")
	}
}

func TestSignedLedgerRevalidatesTransitionsAndContentFreeState(t *testing.T) {
	context := loadExecutionFixture(t)
	seed := sha256.Sum256([]byte("forgeos-adr0058-fixture-execution-receipt-sign-seed-v1"))
	mutations := []func(map[string]any){
		func(ledger map[string]any) {
			entry := ledger["entries"].([]any)[1].(map[string]any)
			entry["execution_policy"] = cloneNode(context.document["execution_policy"])
		},
		func(ledger map[string]any) {
			receipt := ledger["entries"].([]any)[2].(map[string]any)["receipt"].(map[string]any)
			receipt["reservation_receipt_sha256"] = receipt["effect_intent_receipt_sha256"]
			reselfDigest(t, receipt, receiptDomain, "receipt_sha256", maxReceiptBytes, true,
				"", seed[:], receiptSignatureDomain)
		},
		func(ledger map[string]any) {
			entry := ledger["entries"].([]any)[2].(map[string]any)
			entry["result_metadata"] = nil
		},
	}
	for index, mutate := range mutations {
		ledger := cloneNode(context.document["usage_ledger"]).(map[string]any)
		mutate(ledger)
		reselfDigest(t, ledger, ledgerDomain, "ledger_sha256", maxLedgerBytes, true,
			"", seed[:], ledgerSignatureDomain)
		if _, err := validateLedgerDocument(ledger, context.trust); err == nil {
			t.Fatalf("signed invalid Ledger mutation %d was accepted", index)
		}
	}
}

func TestStoredMetadataIdentityAndInvocationTimeoutFailClosed(t *testing.T) {
	context := loadExecutionFixture(t)
	metadata := cloneNode(context.document["result_metadata"]).(map[string]any)
	metadata["execution_result_id"] = "bootstrap-repo-read-result-" +
		strings.Repeat("0", 64)
	reselfDigest(t, metadata, metadataDomain, "metadata_sha256", maxMetadataBytes,
		false, "", nil, nil)
	if err := validateStoredMetadata(&Metadata{metadata}, context.group.manifest); err == nil {
		t.Fatal("stored ResultMetadata with derived identity mismatch was accepted")
	}
	metadata = cloneNode(context.document["result_metadata"]).(map[string]any)
	action := context.group.invocation.document["requested_action"].(map[string]any)
	limit, _ := intValue(action["usage"].(map[string]any), "timeout_ms")
	metadata["observed_usage"].(map[string]any)["elapsed_ms"] = limit + 1
	reselfDigest(t, metadata, metadataDomain, "metadata_sha256", maxMetadataBytes,
		false, "", nil, nil)
	if _, err := replayStoredMetadata(metadata, context.group.terminal.receipt,
		context.group); err == nil {
		t.Fatal("stored ResultMetadata exceeding Invocation timeout was accepted")
	}
}

func reselfDigest(t *testing.T, document map[string]any, domain []byte, field string,
	maximum int, signed bool, idField string, seed, signatureDomain []byte) {
	t.Helper()
	digest, err := selfDigest(domain, document, field, maximum, "test document", signed, idField)
	if err != nil {
		t.Fatal(err)
	}
	document[field] = digest
	if signed {
		signature := document["signature"].(map[string]any)
		signature["signature_base64url"], err = signDigest(seed, signatureDomain, digest)
		if err != nil {
			t.Fatal(err)
		}
	}
}
