package transitionreceiptcontract

import (
	"bytes"
	"testing"
)

func TestGoldenDigestsProjectionAndAssessment(t *testing.T) {
	fixture := loadGolden(t)
	vocabulary := fixtureNode(t, fixture, "transition_vocabulary")
	receipt := fixtureNode(t, fixture, "transition_receipt")
	request := fixtureNode(t, fixture, "assessment_request")
	assessment := fixtureNode(t, fixture, "expected_assessment")
	wants := map[string]string{
		"vocabulary": "cc354fb2b440d81514045b50266d41d3964b6440ed9d40afa17f5991519d7d0d",
		"receipt":    "3d80d9578051338e447f674eedbb856455cd1e672247d88fbba8c51dab9bcb5d",
		"target":     "8be69d5504d243bdb7fedc418c48559055d6639a33edb9aa9b4cb08c3f948d9a",
		"request":    "20e3378571ef708b211ae145dbd285356a1ac05f6dae68784b71562fd95eed7f",
		"assessment": "5e4d62eedecaf2abd9c7f2030466ebc158cefbaa6f01ec21cfebd33db129eb6a",
	}
	assertDigest(t, "vocabulary", vocabulary["vocabulary_sha256"], wants)
	assertDigest(t, "receipt", receipt["receipt_sha256"], wants)
	assertDigest(t, "request", request["request_sha256"], wants)
	assertDigest(t, "assessment", assessment["assessment_sha256"], wants)
	target, err := DeclaredTarget(receipt)
	if err != nil || !canonicalValuesEqual(target, request["expected_target"]) {
		t.Fatalf("target projection drifted: %v", err)
	}
	digest, err := DeclaredTargetSHA256(target)
	if err != nil || digest != wants["target"] {
		t.Fatalf("target digest = %q, %v", digest, err)
	}
	actual, err := AssessDeclared(request)
	if err != nil || !canonicalValuesEqual(actual, assessment) {
		t.Fatalf("assessment reassembly drifted: %v", err)
	}
	if err := ValidateAssessment(request, assessment); err != nil {
		t.Fatal(err)
	}
}

func assertDigest(t *testing.T, key string, actual any, wants map[string]string) {
	t.Helper()
	if actual != wants[key] {
		t.Fatalf("%s digest = %#v", key, actual)
	}
}

func TestGoldenExactCanonicalDecoders(t *testing.T) {
	fixture := loadGolden(t)
	tests := []struct {
		key    string
		encode func(map[string]any) ([]byte, error)
		decode func([]byte) (map[string]any, error)
	}{
		{"transition_vocabulary", CanonicalVocabularyJSON, DecodeCanonicalVocabulary},
		{"transition_receipt", CanonicalReceiptJSON, DecodeCanonicalReceipt},
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
