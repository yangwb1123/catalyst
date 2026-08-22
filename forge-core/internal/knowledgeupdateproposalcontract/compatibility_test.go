package knowledgeupdateproposalcontract

import (
	"encoding/base64"
	"encoding/json"
	"os"
	"testing"

	"forgeos/forge-core/internal/capabilitygrantcontract"
	"forgeos/forge-core/internal/contextpackagecontract"
)

const (
	utf8CounterID   = "forgeos.token-counter.utf8-bytes/v1"
	utf8CounterHash = "44799f99769528ecb46bcad483faf2d8ff4ab086bf32b2fe692a18f0eebea3cf"
)

type byteCounter struct{}

func (byteCounter) Identity() contextpackagecontract.TokenizerIdentity {
	return contextpackagecontract.TokenizerIdentity{
		TokenizerID: utf8CounterID, TokenizerSHA256: utf8CounterHash,
	}
}

func (byteCounter) Count(value []byte) (uint64, error) { return uint64(len(value)), nil }

type contextFixture struct {
	ExpectedPackage contextpackagecontract.ContextPackage `json:"expected_package"`
	Request         contextpackagecontract.BuildRequest   `json:"request"`
}

func loadCapabilityGrant(t *testing.T) map[string]any {
	t.Helper()
	data, err := os.ReadFile("../../../docs/contracts/fixtures/capability-grant-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	root, err := parseStrictJSON(data, maxEnvelopeBytes)
	if err != nil {
		t.Fatal(err)
	}
	return root.(map[string]any)["grant"].(map[string]any)
}

func loadContextFixture(t *testing.T) contextFixture {
	t.Helper()
	data, err := os.ReadFile("../../../docs/contracts/fixtures/context-package-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	var fixture contextFixture
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatal(err)
	}
	return fixture
}

func matchingKnowledgeGrant(t *testing.T, proposal map[string]any) map[string]any {
	t.Helper()
	candidate := cloneNode(loadCapabilityGrant(t))
	candidate["grant_id"] = ""
	candidate["grant_sha256"] = ""
	candidate["authority_proof"].(map[string]any)["proof_base64url"] = ""
	candidate["scope"] = map[string]any{
		"allow": []any{map[string]any{"resources": []any{cloneValue(proposal["knowledge_scope"])}}},
		"deny":  []any{}, "effect_id": "knowledge.propose",
	}
	prepared, _, err := capabilitygrantcontract.PrepareGrantForSigning(candidate)
	if err != nil {
		t.Fatal(err)
	}
	proof := base64.RawURLEncoding.EncodeToString(make([]byte, 64))
	grant, err := capabilitygrantcontract.FinalizeSignedGrant(prepared, proof)
	if err != nil {
		t.Fatal(err)
	}
	return grant
}

func bindProposalToGrant(t *testing.T, proposal, grant map[string]any) {
	t.Helper()
	grantRef, err := ProjectCapabilityGrantRef(grant)
	if err != nil {
		t.Fatal(err)
	}
	proposal["capability_grant_ref"] = grantRef
	resealProposal(t, proposal)
}

func TestGrantCompatibilityMatchesAllDeclaredRelationsWithoutAuthority(t *testing.T) {
	proposal := cloneFixtureObject(t, loadFixture(t), "knowledge_update_proposal")
	grant := matchingKnowledgeGrant(t, proposal)
	bindProposalToGrant(t, proposal, grant)
	assessment, err := AssessDeclaredGrantCompatibility(grant, proposal)
	if err != nil {
		t.Fatal(err)
	}
	if len(assessment["reason_codes"].([]any)) != 0 {
		t.Fatalf("matching reasons got %v", assessment["reason_codes"])
	}
	want := map[string]any{
		"bindings": "same_declared_bindings", "declared_time": "same_declared_time",
		"effect": "same_declared_effect", "grant_ref": "same_declared_grant_ref",
		"proposer": "same_declared_proposer", "scope": "covered_by_declaration",
		"task_binding": "same_declared_task_binding",
	}
	if !canonicalValuesEqual(assessment["relations"], want) {
		t.Fatalf("matching relations got %v", assessment["relations"])
	}
	if assessment["result"] != grantCompatibilityResult {
		t.Fatalf("unexpected result %v", assessment["result"])
	}
}

func TestGrantEffectMismatchForcesOutsideScopeWithoutRedundantScopeReason(t *testing.T) {
	proposal := fixtureObject(t, loadFixture(t), "knowledge_update_proposal")
	grant := loadCapabilityGrant(t)
	assessment, err := AssessDeclaredGrantCompatibility(grant, proposal)
	if err != nil {
		t.Fatal(err)
	}
	relations := assessment["relations"].(map[string]any)
	if relations["effect"] != "effect_mismatch" || relations["scope"] != "outside_declared_scope" {
		t.Fatalf("effect/scope got %v", relations)
	}
	reasons := assessment["reason_codes"].([]any)
	if len(reasons) != 1 || reasons[0] != "effect_mismatch" {
		t.Fatalf("effect mismatch reasons got %v", reasons)
	}
}

func TestGrantDenyPrecedenceIsDeclaredOnly(t *testing.T) {
	proposal := cloneFixtureObject(t, loadFixture(t), "knowledge_update_proposal")
	grant := matchingKnowledgeGrant(t, proposal)
	bindProposalToGrant(t, proposal, grant)
	candidate := cloneNode(grant)
	candidate["grant_id"], candidate["grant_sha256"] = "", ""
	candidate["authority_proof"].(map[string]any)["proof_base64url"] = ""
	candidate["scope"].(map[string]any)["deny"] = []any{cloneValue(proposal["knowledge_scope"])}
	prepared, _, err := capabilitygrantcontract.PrepareGrantForSigning(candidate)
	if err != nil {
		t.Fatal(err)
	}
	grant, err = capabilitygrantcontract.FinalizeSignedGrant(prepared,
		base64.RawURLEncoding.EncodeToString(make([]byte, 64)))
	if err != nil {
		t.Fatal(err)
	}
	bindProposalToGrant(t, proposal, grant)
	assessment, err := AssessDeclaredGrantCompatibility(grant, proposal)
	if err != nil {
		t.Fatal(err)
	}
	if assessment["relations"].(map[string]any)["scope"] != "denied_by_declaration" {
		t.Fatalf("deny relation got %v", assessment["relations"])
	}
	reasons := assessment["reason_codes"].([]any)
	if len(reasons) != 1 || reasons[0] != "deny_matched" {
		t.Fatalf("deny reasons got %v", reasons)
	}
}

func TestContextCompatibilityUsesCallerValidatedPackageDeclarationsOnly(t *testing.T) {
	proposal := cloneFixtureObject(t, loadFixture(t), "knowledge_update_proposal")
	context := loadContextFixture(t)
	matchProposalToContext(t, proposal, &context.ExpectedPackage)
	assessment, err := AssessDeclaredContextCompatibility(&context.Request,
		&context.ExpectedPackage, byteCounter{}, proposal)
	if err != nil {
		t.Fatal(err)
	}
	want := map[string]any{
		"context": "same_declared_context", "freshness": "inside_declared_freshness",
		"policy": "same_declared_policy", "source": "same_declared_source",
		"task_binding": "same_declared_task_binding",
	}
	if !canonicalValuesEqual(assessment["relations"], want) || len(assessment["reason_codes"].([]any)) != 0 {
		t.Fatalf("matching Context compatibility got %v", assessment)
	}
	proposal["submitted_at_unix_ms"] = *context.ExpectedPackage.Freshness.ExpiresAtUnixMS
	resealProposal(t, proposal)
	assessment, err = AssessDeclaredContextCompatibility(&context.Request,
		&context.ExpectedPackage, byteCounter{}, proposal)
	if err != nil {
		t.Fatal(err)
	}
	if assessment["relations"].(map[string]any)["freshness"] != "outside_declared_freshness" ||
		assessment["reason_codes"].([]any)[0] != "freshness_mismatch" {
		t.Fatalf("expired Context compatibility got %v", assessment)
	}
}

func TestContextCompatibilityRejectsPackageThatWasNotExactlyReassembled(t *testing.T) {
	context := loadContextFixture(t)
	proposal := fixtureObject(t, loadFixture(t), "knowledge_update_proposal")
	context.ExpectedPackage.ContextSHA256 = hashOf("0")
	if _, err := AssessDeclaredContextCompatibility(&context.Request,
		&context.ExpectedPackage, byteCounter{}, proposal); err == nil {
		t.Fatal("non-reassembled ContextPackage accepted for declaration comparison")
	}
}

func matchProposalToContext(t *testing.T, proposal map[string]any,
	packageValue *contextpackagecontract.ContextPackage) {
	t.Helper()
	bindings := proposal["bindings"].(map[string]any)
	task := proposal["task_binding"].(map[string]any)
	bindings["context_sha256"] = packageValue.ContextSHA256
	bindings["policy_sha256"] = packageValue.SourceBinding.PolicySHA256
	bindings["source_revision"] = packageValue.SourceBinding.SourceRevision
	bindings["source_tree_sha256"] = packageValue.SourceBinding.SourceTreeSHA256
	task["change_id"], task["node_id"] = packageValue.TaskBinding.ChangeID, packageValue.TaskBinding.NodeID
	task["project_id"], task["role"] = packageValue.TaskBinding.ProjectID, packageValue.TaskBinding.Role
	task["run_id"], task["task_id"] = packageValue.TaskBinding.RunID, packageValue.TaskBinding.TaskID
	for _, mutationValue := range proposal["mutations"].([]any) {
		mutation := mutationValue.(map[string]any)
		afterRef := mutation["after_claim_ref"].(map[string]any)
		record := proposalRecord(proposal, afterRef["record_id"].(string))
		bindAfterRecordToContext(record, bindings, task)
		afterRef["canonical_sha256"] = resealGovernanceRecord(t, record)
	}
	for _, recordValue := range proposal["records"].([]any) {
		recordValue.(map[string]any)["metadata"].(map[string]any)["project_id"] = task["project_id"]
	}
	resealProposalRecords(t, proposal)
}

func bindAfterRecordToContext(record, bindings, task map[string]any) {
	metadata := record["metadata"].(map[string]any)
	metadata["context_sha256"], metadata["policy_sha256"] = bindings["context_sha256"], bindings["policy_sha256"]
	metadata["source_revision"], metadata["source_tree_sha256"] = bindings["source_revision"], bindings["source_tree_sha256"]
	metadata["project_id"] = task["project_id"]
	creator := metadata["created_by"].(map[string]any)
	creator["role"], creator["run_id"] = task["role"], task["run_id"]
}

func proposalRecord(proposal map[string]any, recordID string) map[string]any {
	for _, value := range proposal["records"].([]any) {
		record := value.(map[string]any)
		if record["metadata"].(map[string]any)["record_id"] == recordID {
			return record
		}
	}
	return nil
}
