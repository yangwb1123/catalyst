package transitionreceiptcontract

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

func loadFixtureFile(t *testing.T, name string, maximum int) map[string]any {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", name)
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	raw = bytes.TrimSuffix(raw, []byte("\n"))
	value, err := parseStrictJSON(raw, maximum)
	if err != nil {
		t.Fatal(err)
	}
	fixture, ok := value.(map[string]any)
	if !ok {
		t.Fatal("fixture root is not an object")
	}
	canonical, err := canonicalJSON(fixture)
	if err != nil || !bytes.Equal(canonical, raw) {
		t.Fatal("fixture is not exact canonical JSON")
	}
	return fixture
}

func loadGolden(t *testing.T) map[string]any {
	t.Helper()
	fixture := loadFixtureFile(t, "transition-receipt-v1.json", maxEnvelopeBytes)
	keys := []string{"assessment_request", "expected_approval_refs", "expected_assessment",
		"expected_capability_grant_ref", "transition_receipt", "transition_vocabulary"}
	if err := requireKeys(fixture, keys...); err != nil {
		t.Fatal(err)
	}
	return fixture
}

func fixtureNode(t *testing.T, fixture map[string]any, key string) map[string]any {
	t.Helper()
	node, ok := fixture[key].(map[string]any)
	if !ok {
		t.Fatalf("fixture %s is not an object", key)
	}
	return node
}

func resealReceipt(t *testing.T, receipt map[string]any) {
	t.Helper()
	receipt["receipt_id"] = ""
	receipt["receipt_sha256"] = ""
	if err := validateReceipt(receipt, true); err != nil {
		t.Fatal(err)
	}
	digest, err := receiptDigest(receipt)
	if err != nil {
		t.Fatal(err)
	}
	receipt["receipt_id"] = "transition-receipt-" + digest
	receipt["receipt_sha256"] = digest
	if err := validateReceipt(receipt, false); err != nil {
		t.Fatal(err)
	}
}

func resealRequest(t *testing.T, request map[string]any) {
	t.Helper()
	target := request["expected_target"].(map[string]any)
	targetHash, err := targetDigest(target)
	if err != nil {
		t.Fatal(err)
	}
	request["expected_target_sha256"] = targetHash
	request["request_sha256"] = ""
	digest, err := requestDigest(request)
	if err != nil {
		t.Fatal(err)
	}
	request["request_sha256"] = digest
	if err := validateRequest(request); err != nil {
		t.Fatal(err)
	}
}
