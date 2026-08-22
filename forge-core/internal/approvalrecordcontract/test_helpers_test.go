package approvalrecordcontract

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

func loadGolden(t *testing.T) map[string]any {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures",
		"approval-record-v1.json")
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	raw = bytes.TrimSuffix(raw, []byte("\n"))
	value, err := parseStrictJSON(raw, 4*1024*1024)
	if err != nil {
		t.Fatal(err)
	}
	fixture, ok := value.(map[string]any)
	if !ok || requireKeys(fixture, "approval_record", "assessment_request",
		"expected_approval_ref", "expected_assessment") != nil {
		t.Fatal("golden envelope fields are invalid")
	}
	canonical, err := canonicalJSON(fixture)
	if err != nil || !bytes.Equal(canonical, raw) {
		t.Fatal("golden envelope is not canonical")
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

func resealRecord(t *testing.T, record map[string]any) {
	t.Helper()
	record["approval_id"] = ""
	record["approval_sha256"] = ""
	if err := validateRecord(record, true); err != nil {
		t.Fatal(err)
	}
	digest, err := approvalDigest(record)
	if err != nil {
		t.Fatal(err)
	}
	record["approval_sha256"] = digest
	record["approval_id"] = "approval-record-" + digest
	if err := validateRecord(record, false); err != nil {
		t.Fatal(err)
	}
}
