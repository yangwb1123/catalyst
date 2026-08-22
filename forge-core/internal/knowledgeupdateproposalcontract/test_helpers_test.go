package knowledgeupdateproposalcontract

import (
	"os"
	"testing"
)

func loadFixture(t *testing.T) map[string]any {
	t.Helper()
	data, err := os.ReadFile("../../../docs/contracts/fixtures/knowledge-update-proposal-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	value, err := parseStrictJSON(data, maxEnvelopeBytes)
	if err != nil {
		t.Fatal(err)
	}
	fixture, ok := value.(map[string]any)
	if !ok {
		t.Fatal("fixture root must be an object")
	}
	return fixture
}

func fixtureObject(t *testing.T, fixture map[string]any, key string) map[string]any {
	t.Helper()
	value, err := objectValue(fixture, key)
	if err != nil {
		t.Fatal(err)
	}
	return value
}

func resealProposal(t *testing.T, proposal map[string]any) {
	t.Helper()
	proposal["proposal_id"] = ""
	proposal["proposal_sha256"] = ""
	digest, err := proposalDigest(proposal)
	if err != nil {
		t.Fatal(err)
	}
	proposal["proposal_sha256"] = digest
	proposal["proposal_id"] = "knowledge-update-proposal-" + digest
}

func resealRequest(t *testing.T, request map[string]any) {
	t.Helper()
	request["request_sha256"] = ""
	digest, err := requestDigest(request)
	if err != nil {
		t.Fatal(err)
	}
	request["request_sha256"] = digest
}

func cloneFixtureObject(t *testing.T, fixture map[string]any, key string) map[string]any {
	t.Helper()
	return cloneNode(fixtureObject(t, fixture, key))
}

func resealGovernanceRecord(t *testing.T, record map[string]any) string {
	t.Helper()
	integrity := record["integrity"].(map[string]any)
	integrity["canonical_sha256"] = ""
	domain := "forgeos.governance.knowledge-claim.v1\x00"
	if record["kind"] == "EvidenceRecord" {
		domain = "forgeos.governance.evidence-record.v1\x00"
	}
	digest, err := digestValue(domain, record)
	if err != nil {
		t.Fatal(err)
	}
	integrity["canonical_sha256"] = digest
	return digest
}

func resealProposalRecords(t *testing.T, proposal map[string]any) {
	t.Helper()
	records := proposal["records"].([]any)
	digest, err := digestValue(recordSetDomain, records)
	if err != nil {
		t.Fatal(err)
	}
	proposal["record_set_sha256"] = digest
	resealProposal(t, proposal)
}

func resealTargetRequest(t *testing.T, request map[string]any) {
	t.Helper()
	target := request["expected_target"].(map[string]any)
	digest, err := targetDigest(target)
	if err != nil {
		t.Fatal(err)
	}
	request["expected_target_sha256"] = digest
	resealRequest(t, request)
}
