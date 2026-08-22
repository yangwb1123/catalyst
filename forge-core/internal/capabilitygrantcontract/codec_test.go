package capabilitygrantcontract

import (
	"bytes"
	"testing"
)

func TestCanonicalDecodersRejectWireDrift(t *testing.T) {
	fixture := loadFixture(t)
	vocabulary := fixtureNode(t, fixture, "effect_vocabulary")
	canonical, err := CanonicalVocabularyJSON(vocabulary)
	if err != nil {
		t.Fatal(err)
	}
	duplicate := bytes.Replace(canonical, []byte(`{"api_version":`),
		[]byte(`{"api_version":"forgeos.governance.effect-vocabulary/v1","api_version":`), 1)
	cases := [][]byte{append(append([]byte(nil), canonical...), '\n'), duplicate,
		bytes.Replace(canonical, []byte("EffectVocabulary"), []byte(`Effect\u0056ocabulary`), 1)}
	for index, data := range cases {
		if _, err := DecodeCanonicalVocabulary(data); err == nil {
			t.Fatalf("wire drift case %d was accepted", index)
		}
	}
	grant := fixtureNode(t, fixture, "grant")
	grantJSON, err := CanonicalGrantJSON(grant)
	if err != nil {
		t.Fatal(err)
	}
	invalidNumbers := [][]byte{
		bytes.Replace(grantJSON, []byte(`"trust_epoch":1`), []byte(`"trust_epoch":1.0`), 1),
		bytes.Replace(grantJSON, []byte(`"trust_epoch":1`), []byte(`"trust_epoch":9223372036854775808`), 1),
	}
	for _, data := range invalidNumbers {
		if _, err := DecodeCanonicalGrant(data); err == nil {
			t.Fatal("non-int64 JSON number was accepted")
		}
	}
}

func TestCanonicalEncoderDoesNotHTMLEscape(t *testing.T) {
	encoded, err := canonicalJSON(map[string]any{"value": "<forge&grant>"})
	if err != nil {
		t.Fatal(err)
	}
	if string(encoded) != `{"value":"<forge&grant>"}` {
		t.Fatalf("unexpected canonical Unicode encoding: %s", encoded)
	}
}

func TestFrozenVocabularyRejectsSelfConsistentSubstitution(t *testing.T) {
	fixture := loadFixture(t)
	vocabulary := cloneNode(fixtureNode(t, fixture, "effect_vocabulary"))
	effects, _ := arrayValue(vocabulary, "effects")
	descriptor := effects[0].(map[string]any)
	descriptor["production_restriction"] = "external_operator_only"
	vocabulary["vocabulary_sha256"] = ""
	forgedDigest, err := digestNode(vocabularyDigestDomain, vocabulary)
	if err != nil {
		t.Fatal(err)
	}
	vocabulary["vocabulary_sha256"] = forgedDigest
	encoded, _ := canonicalJSON(vocabulary)
	if _, err := DecodeCanonicalVocabulary(encoded); err == nil {
		t.Fatal("a substituted and self-rehashed effect vocabulary was accepted")
	}
}

func TestAuthorityEscalationIsRejected(t *testing.T) {
	fixture := loadFixture(t)
	assessment := cloneNode(fixtureNode(t, fixture, "expected_assessment"))
	assessment["permission_attestation"] = true
	resealAssessment(t, assessment)
	encoded, _ := canonicalJSON(assessment)
	if _, err := DecodeCanonicalAssessment(encoded); err == nil {
		t.Fatal("permission escalation was accepted")
	}
	request := cloneNode(fixtureNode(t, fixture, "assessment_request"))
	grant := fixtureNode(t, request, "grant")
	proof := fixtureNode(t, grant, "authority_proof")
	issuer := fixtureNode(t, proof, "issuer")
	issuer["principal_type"] = "agent"
	resealGrant(t, grant)
	resealRequest(t, request)
	requestJSON, _ := canonicalJSON(request)
	if _, err := DecodeCanonicalAssessmentRequest(requestJSON); err == nil {
		t.Fatal("agent issuer escalation was accepted")
	}
}

func TestAssessmentDoesNotMutateInputs(t *testing.T) {
	fixture := loadFixture(t)
	vocabulary := fixtureNode(t, fixture, "effect_vocabulary")
	request := fixtureNode(t, fixture, "assessment_request")
	beforeVocabulary, _ := canonicalJSON(vocabulary)
	beforeRequest, _ := canonicalJSON(request)
	if _, err := AssessDeclared(vocabulary, request); err != nil {
		t.Fatal(err)
	}
	afterVocabulary, _ := canonicalJSON(vocabulary)
	afterRequest, _ := canonicalJSON(request)
	if !bytes.Equal(beforeVocabulary, afterVocabulary) || !bytes.Equal(beforeRequest, afterRequest) {
		t.Fatal("assessment mutated caller-owned inputs")
	}
}
