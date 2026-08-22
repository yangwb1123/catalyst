package knowledgeupdateproposalcontract

import (
	"bytes"
	"strings"
	"testing"
)

func TestCanonicalDecoderRejectsWireDriftAndAuthorityFields(t *testing.T) {
	fixture := loadFixture(t)
	proposal := fixtureObject(t, fixture, "knowledge_update_proposal")
	canonical, err := CanonicalProposalJSON(proposal)
	if err != nil {
		t.Fatal(err)
	}
	unknown := cloneNode(proposal)
	for _, key := range []string{"applied", "authorized", "current_knowledge_state", "receipt"} {
		unknown[key] = false
		if _, err := CanonicalProposalJSON(unknown); err == nil {
			t.Fatalf("authority-like unknown field %q accepted", key)
		}
		delete(unknown, key)
	}
	if _, err := DecodeCanonicalProposal(append([]byte(" "), canonical...)); err == nil {
		t.Fatal("noncanonical whitespace accepted")
	}
	duplicate := append([]byte(`{"api_version":"forgeos.knowledge-update-proposal/v1",`), canonical[1:]...)
	if _, err := DecodeCanonicalProposal(duplicate); err == nil {
		t.Fatal("duplicate JSON key accepted")
	}
	floatTime := bytes.Replace(canonical, []byte(`"submitted_at_unix_ms":1700000002000`),
		[]byte(`"submitted_at_unix_ms":1700000002000.0`), 1)
	if _, err := DecodeCanonicalProposal(floatTime); err == nil {
		t.Fatal("floating point time accepted")
	}
}

func TestMutationAliasesTargetsOrderAndForkFailClosed(t *testing.T) {
	fixture := loadFixture(t)
	for _, operation := range []string{"apply", "update", "upsert", "delete", "retract"} {
		proposal := cloneFixtureObject(t, fixture, "knowledge_update_proposal")
		proposal["mutations"].([]any)[0].(map[string]any)["operation"] = operation
		resealProposal(t, proposal)
		if _, err := CanonicalProposalJSON(proposal); err == nil {
			t.Fatalf("operation alias %q accepted", operation)
		}
	}
	proposal := cloneFixtureObject(t, fixture, "knowledge_update_proposal")
	mutations := proposal["mutations"].([]any)
	mutations[0], mutations[1] = mutations[1], mutations[0]
	resealProposal(t, proposal)
	if _, err := CanonicalProposalJSON(proposal); err == nil {
		t.Fatal("out-of-order mutations accepted")
	}

	proposal = cloneFixtureObject(t, fixture, "knowledge_update_proposal")
	mutations = proposal["mutations"].([]any)
	create := mutations[0].(map[string]any)
	revise := mutations[1].(map[string]any)
	create["operation"] = "supersede"
	create["before_claim_ref"] = cloneValue(revise["before_claim_ref"])
	resealProposal(t, proposal)
	if _, err := CanonicalProposalJSON(proposal); err == nil {
		t.Fatal("forked before_claim_ref accepted")
	}
}

func TestMutationReasonCodesUseExactADR0045IdentifierGrammar(t *testing.T) {
	proposal := cloneFixtureObject(t, loadFixture(t), "knowledge_update_proposal")
	proposal["mutations"].([]any)[0].(map[string]any)["reason_codes"] = []any{"1:declared/reason"}
	resealProposal(t, proposal)
	if _, err := CanonicalProposalJSON(proposal); err != nil {
		t.Fatalf("valid ADR-0045 reason identifier rejected: %v", err)
	}
	proposal = cloneFixtureObject(t, loadFixture(t), "knowledge_update_proposal")
	proposal["mutations"].([]any)[0].(map[string]any)["reason_codes"] = []any{"invalid reason"}
	resealProposal(t, proposal)
	if _, err := CanonicalProposalJSON(proposal); err == nil {
		t.Fatal("reason outside ADR-0045 identifier grammar accepted")
	}
}

func TestMutationReferenceAndCreateLifecycleRulesFailClosed(t *testing.T) {
	fixture := loadFixture(t)
	proposal := cloneFixtureObject(t, fixture, "knowledge_update_proposal")
	mutation := proposal["mutations"].([]any)[0].(map[string]any)
	mutation["before_claim_ref"] = cloneValue(proposal["mutations"].([]any)[1].(map[string]any)["before_claim_ref"])
	resealProposal(t, proposal)
	if _, err := CanonicalProposalJSON(proposal); err == nil {
		t.Fatal("create with before Claim accepted")
	}

	proposal = cloneFixtureObject(t, fixture, "knowledge_update_proposal")
	mutation = proposal["mutations"].([]any)[1].(map[string]any)
	evidence := proposal["records"].([]any)[3].(map[string]any)
	mutation["after_claim_ref"] = map[string]any{
		"canonical_sha256": evidence["integrity"].(map[string]any)["canonical_sha256"],
		"record_id":        evidence["metadata"].(map[string]any)["record_id"],
	}
	resealProposal(t, proposal)
	if _, err := CanonicalProposalJSON(proposal); err == nil {
		t.Fatal("EvidenceRecord mutation target accepted")
	}

	proposal = cloneFixtureObject(t, fixture, "knowledge_update_proposal")
	mutation = proposal["mutations"].([]any)[1].(map[string]any)
	mutation["after_claim_ref"].(map[string]any)["canonical_sha256"] = strings.Repeat("0", 64)
	resealProposal(t, proposal)
	if _, err := CanonicalProposalJSON(proposal); err == nil {
		t.Fatal("incorrect Claim ref digest accepted")
	}
}

func TestDeclaredTargetAndRequestRejectSameBeforeAndAfterRecord(t *testing.T) {
	fixture := loadFixture(t)
	target := cloneNode(fixtureObject(t, fixture, "assessment_request")["expected_target"].(map[string]any))
	mutation := target["mutations"].([]any)[1].(map[string]any)
	mutation["before_claim_ref"] = cloneValue(mutation["after_claim_ref"])
	if _, err := CanonicalDeclaredTargetJSON(target); err == nil ||
		!strings.Contains(err.Error(), "must identify different records") {
		t.Fatalf("declared target same-ref result: %v", err)
	}

	request := cloneFixtureObject(t, fixture, "assessment_request")
	mutation = request["expected_target"].(map[string]any)["mutations"].([]any)[1].(map[string]any)
	mutation["before_claim_ref"] = cloneValue(mutation["after_claim_ref"])
	if _, err := CanonicalAssessmentRequestJSON(request); err == nil ||
		!strings.Contains(err.Error(), "must identify different records") {
		t.Fatalf("assessment request same-ref result: %v", err)
	}
}

func TestDeclaredTargetAndRequestRejectCrossMutationBeforeAfterOverlap(t *testing.T) {
	fixture := loadFixture(t)
	target := cloneNode(fixtureObject(t, fixture, "assessment_request")["expected_target"].(map[string]any))
	mutations := target["mutations"].([]any)
	mutations[1].(map[string]any)["before_claim_ref"] = cloneValue(
		mutations[0].(map[string]any)["after_claim_ref"])
	if _, err := CanonicalDeclaredTargetJSON(target); err == nil ||
		!strings.Contains(err.Error(), "cannot also be used as a before_claim_ref") {
		t.Fatalf("declared target cross-mutation overlap result: %v", err)
	}

	request := cloneFixtureObject(t, fixture, "assessment_request")
	mutations = request["expected_target"].(map[string]any)["mutations"].([]any)
	mutations[1].(map[string]any)["before_claim_ref"] = cloneValue(
		mutations[0].(map[string]any)["after_claim_ref"])
	if _, err := CanonicalAssessmentRequestJSON(request); err == nil ||
		!strings.Contains(err.Error(), "cannot also be used as a before_claim_ref") {
		t.Fatalf("assessment request cross-mutation overlap result: %v", err)
	}
}

func TestExactClosureRejectsOrphanRecord(t *testing.T) {
	proposal := cloneFixtureObject(t, loadFixture(t), "knowledge_update_proposal")
	proposal["mutations"] = proposal["mutations"].([]any)[1:]
	resealProposal(t, proposal)
	if _, err := CanonicalProposalJSON(proposal); err == nil || !strings.Contains(err.Error(), "outside the exact mutation closure") {
		t.Fatalf("orphan closure result: %v", err)
	}
}

func TestSupersedeRejectsSemanticIdentityAndLifecycleDrift(t *testing.T) {
	fixture := loadFixture(t)
	proposal := cloneFixtureObject(t, fixture, "knowledge_update_proposal")
	records := proposal["records"].([]any)
	after := records[0].(map[string]any)
	after["spec"].(map[string]any)["object_value"] = "drifted semantic value"
	afterDigest := resealGovernanceRecord(t, after)
	proposal["mutations"].([]any)[1].(map[string]any)["after_claim_ref"].(map[string]any)["canonical_sha256"] = afterDigest
	resealProposalRecords(t, proposal)
	if _, err := CanonicalProposalJSON(proposal); err == nil || !strings.Contains(err.Error(), "stable semantic") {
		t.Fatalf("semantic drift result: %v", err)
	}

	proposal = cloneFixtureObject(t, fixture, "knowledge_update_proposal")
	records = proposal["records"].([]any)
	after = records[0].(map[string]any)
	before := records[1].(map[string]any)
	after["spec"].(map[string]any)["claim_type"] = "proposal"
	before["spec"].(map[string]any)["claim_type"] = "proposal"
	after["status"].(map[string]any)["state"] = "draft"
	before["status"].(map[string]any)["state"] = "submitted"
	afterDigest = resealGovernanceRecord(t, after)
	beforeDigest := resealGovernanceRecord(t, before)
	mutation := proposal["mutations"].([]any)[1].(map[string]any)
	mutation["after_claim_ref"].(map[string]any)["canonical_sha256"] = afterDigest
	mutation["before_claim_ref"].(map[string]any)["canonical_sha256"] = beforeDigest
	resealProposalRecords(t, proposal)
	if _, err := CanonicalProposalJSON(proposal); err == nil || !strings.Contains(err.Error(), "shadow lifecycle") {
		t.Fatalf("reverse proposal lifecycle result: %v", err)
	}
}

func TestSupersedeMayBindOlderHistoryWhileNamingExactImmediatePredecessor(t *testing.T) {
	proposal := cloneFixtureObject(t, loadFixture(t), "knowledge_update_proposal")
	records := proposal["records"].([]any)
	after := records[0].(map[string]any)
	before := records[1].(map[string]any)
	ancestor := cloneNode(before)
	ancestorMeta := ancestor["metadata"].(map[string]any)
	ancestorMeta["record_id"] = "claim-knowledge-update-ancestor"
	ancestorDigest := resealGovernanceRecord(t, ancestor)
	beforeMeta := before["metadata"].(map[string]any)
	beforeMeta["sequence"] = int64(2)
	beforeMeta["supersedes_record_ids"] = []any{"claim-knowledge-update-ancestor"}
	beforeDigest := resealGovernanceRecord(t, before)
	afterMeta := after["metadata"].(map[string]any)
	afterMeta["sequence"] = int64(3)
	afterMeta["supersedes_record_ids"] = []any{
		"claim-knowledge-update-ancestor", "claim-knowledge-update-before",
	}
	afterDigest := resealGovernanceRecord(t, after)
	proposal["records"] = append(records[:1], append([]any{ancestor}, records[1:]...)...)
	mutation := proposal["mutations"].([]any)[1].(map[string]any)
	mutation["after_claim_ref"].(map[string]any)["canonical_sha256"] = afterDigest
	mutation["before_claim_ref"].(map[string]any)["canonical_sha256"] = beforeDigest
	resealProposalRecords(t, proposal)
	if _, err := CanonicalProposalJSON(proposal); err != nil {
		t.Fatalf("legal older-history membership rejected: %v (ancestor %s)", err, ancestorDigest)
	}
}

func TestProgrammaticCyclesAndByteCeilingsReturnErrorsWithoutPanic(t *testing.T) {
	proposal := cloneFixtureObject(t, loadFixture(t), "knowledge_update_proposal")
	bindings := proposal["bindings"].(map[string]any)
	bindings["cycle"] = bindings
	if _, err := CanonicalProposalJSON(proposal); err == nil {
		t.Fatal("cyclic programmatic proposal accepted")
	}

	proposal = cloneFixtureObject(t, loadFixture(t), "knowledge_update_proposal")
	huge := make([]any, maxArrayItems)
	for index := range huge {
		huge[index] = strings.Repeat("x", maxStringBytes)
	}
	proposal["oversized"] = huge
	if _, err := CanonicalProposalJSON(proposal); err == nil || !strings.Contains(err.Error(), "configured byte limit") {
		t.Fatalf("oversized proposal result: %v", err)
	}
}
