package knowledgeupdateproposalcontract

import (
	"bytes"
	"testing"
)

const (
	goldenRecordSet  = "c14c11c126c1b76ac1affb3421f2ffea20f5c8567fc43f9caef7bed3683c5c7f"
	goldenProposal   = "a4c08d011e3bfb6c08e9d9f5806f39830406478c16f93bad6c8ecde5d3b519b1"
	goldenTarget     = "34e367580f5f2ddbf780911d8fb6d73e89949f0231f220444537e30b49eeff85"
	goldenRequest    = "d0c325f29617e3a164fec4f897c31bbee2bec316c008ba52740477290c05b413"
	goldenAssessment = "e30a494f0e911cf1b312babd1b296786da00760f797857f7b4f0697fa506b037"
)

func TestGoldenExactCanonicalBytesAndFiveDigests(t *testing.T) {
	fixture := loadFixture(t)
	proposal := fixtureObject(t, fixture, "knowledge_update_proposal")
	proposalJSON, err := CanonicalProposalJSON(proposal)
	if err != nil {
		t.Fatal(err)
	}
	decodedProposal, err := DecodeCanonicalProposal(proposalJSON)
	if err != nil || !canonicalValuesEqual(proposal, decodedProposal) {
		t.Fatalf("proposal exact round trip: %v", err)
	}
	records := proposal["records"].([]any)
	recordSetHash, err := RecordSetSHA256(records)
	if err != nil || recordSetHash != goldenRecordSet {
		t.Fatalf("record set digest got %q: %v", recordSetHash, err)
	}
	proposalHash, err := KnowledgeUpdateProposalSHA256(proposal)
	if err != nil || proposalHash != goldenProposal {
		t.Fatalf("proposal digest got %q: %v", proposalHash, err)
	}
	request := fixtureObject(t, fixture, "assessment_request")
	target, err := DeclaredTarget(proposal)
	if err != nil || !canonicalValuesEqual(target, request["expected_target"]) {
		t.Fatalf("target projection: %v", err)
	}
	targetHash, err := DeclaredTargetSHA256(target)
	if err != nil || targetHash != goldenTarget {
		t.Fatalf("target digest got %q: %v", targetHash, err)
	}
	assertGoldenAssessment(t, request, fixtureObject(t, fixture, "expected_assessment"))
	if bytes.Contains(proposalJSON, []byte(`"applied"`)) {
		t.Fatal("golden escalates application semantics")
	}
}

func assertGoldenAssessment(t *testing.T, request, expectedAssessment map[string]any) {
	t.Helper()
	requestJSON, err := CanonicalAssessmentRequestJSON(request)
	if err != nil {
		t.Fatal(err)
	}
	decodedRequest, err := DecodeCanonicalAssessmentRequest(requestJSON)
	if err != nil || !canonicalValuesEqual(request, decodedRequest) {
		t.Fatalf("request exact round trip: %v", err)
	}
	requestHash, err := AssessmentRequestSHA256(request)
	if err != nil || requestHash != goldenRequest {
		t.Fatalf("request digest got %q: %v", requestHash, err)
	}
	assessment, err := AssessDeclared(request)
	if err != nil || !canonicalValuesEqual(assessment, expectedAssessment) {
		t.Fatalf("assessment reproduction: %v", err)
	}
	if assessment["assessment_sha256"] != goldenAssessment {
		t.Fatalf("assessment digest got %v", assessment["assessment_sha256"])
	}
	assessmentHash, err := AssessmentSHA256(assessment)
	if err != nil || assessmentHash != goldenAssessment {
		t.Fatalf("assessment digest helper got %q: %v", assessmentHash, err)
	}
	assessmentJSON, err := CanonicalAssessmentJSON(assessment)
	if err != nil {
		t.Fatal(err)
	}
	decodedAssessment, err := DecodeCanonicalAssessment(assessmentJSON)
	if err != nil || !canonicalValuesEqual(assessment, decodedAssessment) {
		t.Fatalf("assessment exact round trip: %v", err)
	}
	if err := ValidateAssessment(request, assessment); err != nil {
		t.Fatal(err)
	}
	if bytes.Contains(assessmentJSON, []byte(`"authorized"`)) {
		t.Fatal("golden escalates authority semantics")
	}
}

func TestGoldenArtifactProjection(t *testing.T) {
	fixture := loadFixture(t)
	proposal := fixtureObject(t, fixture, "knowledge_update_proposal")
	expected := fixture["expected_artifact_resources"]
	resources, err := ProjectArtifactResources(proposal)
	if err != nil || !canonicalValuesEqual(resources, expected) {
		t.Fatalf("artifact projection: %v", err)
	}
}
