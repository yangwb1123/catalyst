package approvalrecordcontract

import (
	"bytes"
	"testing"
)

func TestGoldenDigestsProjectionAndAssessment(t *testing.T) {
	fixture := loadGolden(t)
	record := fixtureNode(t, fixture, "approval_record")
	request := fixtureNode(t, fixture, "assessment_request")
	assessment := fixtureNode(t, fixture, "expected_assessment")
	wants := map[string]string{
		"approval":   "a2c47ec0c9242d9088532ce58140643a11b3a28f43836134ed36c2c9e2ca09d4",
		"target":     "8402062537970279a1a2cff83913131656e9da341c593918281742850c646f6c",
		"request":    "c90f6108ade8e9066e907bb09a4d5b7ace848e0b9da3be9ee718ccfbc39d9f33",
		"assessment": "1719084506446d2979d4294e53f3a4541200b35d6ac103660b2861df75f786d4",
	}
	recordHash, err := ApprovalRecordSHA256(record)
	if err != nil || recordHash != wants["approval"] {
		t.Fatalf("approval digest = %q, %v", recordHash, err)
	}
	target, err := DeclaredTarget(record)
	if err != nil {
		t.Fatal(err)
	}
	targetHash, err := DeclaredTargetSHA256(target)
	if err != nil || targetHash != wants["target"] {
		t.Fatalf("target digest = %q, %v", targetHash, err)
	}
	if request["request_sha256"] != wants["request"] ||
		assessment["assessment_sha256"] != wants["assessment"] {
		t.Fatal("golden request or assessment digest drifted")
	}
	actual, err := AssessDeclared(request)
	if err != nil || !canonicalValuesEqual(actual, assessment) {
		t.Fatalf("assessment reassembly drifted: %v", err)
	}
	if err := ValidateAssessment(request, assessment); err != nil {
		t.Fatal(err)
	}
}

func TestGoldenExactCanonicalDecoders(t *testing.T) {
	fixture := loadGolden(t)
	tests := []struct {
		key    string
		encode func(map[string]any) ([]byte, error)
		decode func([]byte) (map[string]any, error)
	}{
		{"approval_record", CanonicalRecordJSON, DecodeCanonicalRecord},
		{"assessment_request", CanonicalAssessmentRequestJSON, DecodeCanonicalAssessmentRequest},
		{"expected_assessment", CanonicalAssessmentJSON, DecodeCanonicalAssessment},
	}
	for _, test := range tests {
		node := fixtureNode(t, fixture, test.key)
		encoded, err := test.encode(node)
		if err != nil {
			t.Fatalf("%s encode: %v", test.key, err)
		}
		decoded, err := test.decode(encoded)
		if err != nil || !canonicalValuesEqual(decoded, node) {
			t.Fatalf("%s decode: %v", test.key, err)
		}
		if _, err := test.decode(append(bytes.Clone(encoded), '\n')); err == nil {
			t.Fatalf("%s accepted trailing newline", test.key)
		}
	}
	target := fixtureNode(t, fixtureNode(t, fixture, "assessment_request"), "expected_target")
	encoded, err := CanonicalDeclaredTargetJSON(target)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := DecodeCanonicalDeclaredTarget(encoded); err != nil {
		t.Fatal(err)
	}
}

func TestGoldenApprovalRefIsOnlyADeclaredReference(t *testing.T) {
	fixture := loadGolden(t)
	record := fixtureNode(t, fixture, "approval_record")
	expected := fixtureNode(t, fixture, "expected_approval_ref")
	actual, err := ApprovalRef(record)
	if err != nil || !canonicalValuesEqual(actual, expected) {
		t.Fatalf("ApprovalRef projection drifted: %v", err)
	}
	relation, err := ApprovalRefRelation(record, expected)
	if err != nil || relation != "same_declared_reference" {
		t.Fatalf("ApprovalRef relation = %q, %v", relation, err)
	}
	mismatch := cloneNode(expected)
	mismatch["authority_domain"] = "other.authority"
	relation, err = ApprovalRefRelation(record, mismatch)
	if err != nil || relation != "reference_mismatch" {
		t.Fatalf("mismatch relation = %q, %v", relation, err)
	}
}
