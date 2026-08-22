package capabilitygrantcontract

import (
	"os"
	"testing"
)

func loadFixture(t *testing.T) map[string]any {
	t.Helper()
	data, err := os.ReadFile("../../../docs/contracts/fixtures/capability-grant-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	value, err := parseStrictJSON(data, maxAssessmentRequestBytes)
	if err != nil {
		t.Fatal(err)
	}
	fixture, ok := value.(map[string]any)
	if !ok {
		t.Fatal("fixture root is not an object")
	}
	return fixture
}

func fixtureNode(t *testing.T, fixture map[string]any, key string) map[string]any {
	t.Helper()
	node, err := objectValue(fixture, key)
	if err != nil {
		t.Fatal(err)
	}
	return node
}

func resealGrant(t *testing.T, grant map[string]any) {
	t.Helper()
	grant["grant_id"] = ""
	grant["grant_sha256"] = ""
	proof, err := objectValue(grant, "authority_proof")
	if err != nil {
		t.Fatal(err)
	}
	originalProof := proof["proof_base64url"]
	proof["proof_base64url"] = ""
	digest, err := digestNode(grantDigestDomain, grant)
	if err != nil {
		t.Fatal(err)
	}
	proof["proof_base64url"] = originalProof
	grant["grant_sha256"] = digest
	grant["grant_id"] = "capability-grant-" + digest
}

func resealRequest(t *testing.T, request map[string]any) {
	t.Helper()
	request["request_sha256"] = ""
	digest, err := digestNode(requestDigestDomain, request)
	if err != nil {
		t.Fatal(err)
	}
	request["request_sha256"] = digest
}

func resealAssessment(t *testing.T, assessment map[string]any) {
	t.Helper()
	assessment["assessment_sha256"] = ""
	digest, err := digestNode(assessmentDigestDomain, assessment)
	if err != nil {
		t.Fatal(err)
	}
	assessment["assessment_sha256"] = digest
}
