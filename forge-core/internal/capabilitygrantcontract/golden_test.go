package capabilitygrantcontract

import "testing"

func TestGoldenFixtureIsAssessedDeclarationsOnly(t *testing.T) {
	fixture := loadFixture(t)
	vocabulary := fixtureNode(t, fixture, "effect_vocabulary")
	grant := fixtureNode(t, fixture, "grant")
	request := fixtureNode(t, fixture, "assessment_request")
	expected := fixtureNode(t, fixture, "expected_assessment")
	if err := validateVocabulary(vocabulary); err != nil {
		t.Fatalf("vocabulary: %v", err)
	}
	if err := validateGrant(grant); err != nil {
		t.Fatalf("grant: %v", err)
	}
	if err := validateAssessmentRequest(request); err != nil {
		t.Fatalf("request: %v", err)
	}
	if err := validateAssessment(expected); err != nil {
		t.Fatalf("assessment: %v", err)
	}
	actual, err := AssessDeclared(vocabulary, request)
	if err != nil {
		t.Fatal(err)
	}
	if !canonicalValuesEqual(actual, expected) {
		t.Fatal("Go assessment differs from cross-language golden")
	}
	if err := ValidateAssessment(vocabulary, request, expected); err != nil {
		t.Fatal(err)
	}
}

func TestGoldenExactDigestsAndRootGrant(t *testing.T) {
	fixture := loadFixture(t)
	request := fixtureNode(t, fixture, "assessment_request")
	rootGrant := fixtureNode(t, fixture, "grant")
	nestedGrant := fixtureNode(t, request, "grant")
	if !canonicalValuesEqual(rootGrant, nestedGrant) {
		t.Fatal("root grant and assessment request grant differ")
	}
	expected := map[string]string{
		"vocabulary_sha256": frozenVocabularySHA256,
		"grant_sha256":      "892fd08c827835a3d7e742bda656cd3abf78e7757248e1ac84583715146250c3",
		"request_sha256":    "192d46339703d90b8b19fc8dcd08ded549236cd83ad942019922218d71576f8b",
		"assessment_sha256": "ae8784d3f2cbe296e5968f9e4adbd7d696e956b7424dfc7abf75ba838540f94d",
	}
	checks := map[string]map[string]any{
		"vocabulary_sha256": fixtureNode(t, fixture, "effect_vocabulary"),
		"grant_sha256":      rootGrant, "request_sha256": request,
		"assessment_sha256": fixtureNode(t, fixture, "expected_assessment"),
	}
	for key, node := range checks {
		if node[key] != expected[key] {
			t.Fatalf("%s mismatch: %v", key, node[key])
		}
	}
	action := fixtureNode(t, request, "requested_action")
	digest, err := requestedActionDigest(action)
	if err != nil || digest != "6b5e12d76919b3ed5aab0f235f7b5bd569232d376fa9e6498f80f569c6ab7f11" {
		t.Fatalf("requested action digest mismatch: %s (%v)", digest, err)
	}
}

func TestCanonicalDecodersRoundTripGolden(t *testing.T) {
	fixture := loadFixture(t)
	cases := []struct {
		key    string
		encode func(map[string]any) ([]byte, error)
		decode func([]byte) (map[string]any, error)
	}{
		{"effect_vocabulary", CanonicalVocabularyJSON, DecodeCanonicalVocabulary},
		{"grant", CanonicalGrantJSON, DecodeCanonicalGrant},
		{"assessment_request", CanonicalAssessmentRequestJSON, DecodeCanonicalAssessmentRequest},
		{"expected_assessment", CanonicalAssessmentJSON, DecodeCanonicalAssessment},
	}
	for _, testCase := range cases {
		node := fixtureNode(t, fixture, testCase.key)
		encoded, err := testCase.encode(node)
		if err != nil {
			t.Fatalf("%s encode: %v", testCase.key, err)
		}
		decoded, err := testCase.decode(encoded)
		if err != nil || !canonicalValuesEqual(decoded, node) {
			t.Fatalf("%s decode: %v", testCase.key, err)
		}
	}
}
