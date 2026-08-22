package capabilitygrantcontract

import (
	"bytes"
	"fmt"
)

type documentValidator func(map[string]any) error

// DecodeCanonicalVocabulary strictly decodes the frozen EffectVocabulary v1.
func DecodeCanonicalVocabulary(data []byte) (map[string]any, error) {
	return decodeCanonicalDocument(data, maxVocabularyBytes, validateVocabulary)
}

// DecodeCanonicalGrant strictly decodes a contract-only CapabilityGrant v1.
func DecodeCanonicalGrant(data []byte) (map[string]any, error) {
	return decodeCanonicalDocument(data, maxGrantBytes, validateGrant)
}

// DecodeCanonicalAssessmentRequest decodes an explicit declared-assessment request.
func DecodeCanonicalAssessmentRequest(data []byte) (map[string]any, error) {
	return decodeCanonicalDocument(data, maxAssessmentRequestBytes, validateAssessmentRequest)
}

// DecodeCanonicalAssessment decodes an authority-neutral assessment envelope.
func DecodeCanonicalAssessment(data []byte) (map[string]any, error) {
	return decodeCanonicalDocument(data, maxAssessmentBytes, validateAssessment)
}

// CanonicalVocabularyJSON validates and encodes a vocabulary without mutation.
func CanonicalVocabularyJSON(vocabulary map[string]any) ([]byte, error) {
	return encodeValidated(vocabulary, validateVocabulary)
}

// CanonicalGrantJSON validates and encodes a grant without mutation.
func CanonicalGrantJSON(grant map[string]any) ([]byte, error) {
	return encodeValidated(grant, validateGrant)
}

// CanonicalAssessmentRequestJSON validates and encodes a request without mutation.
func CanonicalAssessmentRequestJSON(request map[string]any) ([]byte, error) {
	return encodeValidated(request, validateAssessmentRequest)
}

// CanonicalAssessmentJSON validates and encodes an assessment without mutation.
func CanonicalAssessmentJSON(assessment map[string]any) ([]byte, error) {
	return encodeValidated(assessment, validateAssessment)
}

// CanonicalRequestedActionJSON validates and encodes the frozen requested-action shape.
func CanonicalRequestedActionJSON(action map[string]any) ([]byte, error) {
	return encodeValidated(action, validateRequestedAction)
}

// RequestedActionSHA256 returns the frozen domain-separated action digest.
func RequestedActionSHA256(action map[string]any) (string, error) {
	if err := validateRequestedAction(action); err != nil {
		return "", err
	}
	return requestedActionDigest(action)
}

// AssessDeclared compares only self-declared envelope relations. It never
// authenticates an issuer or asserts authorization, approval, or permission.
func AssessDeclared(vocabulary, request map[string]any) (map[string]any, error) {
	return assessDeclared(vocabulary, request)
}

// ValidateAssessment regenerates the assessment from current inputs.
func ValidateAssessment(vocabulary, request, assessment map[string]any) error {
	if err := validateAssessment(assessment); err != nil {
		return err
	}
	return validateAssessmentAgainstRequest(vocabulary, request, assessment)
}

func decodeCanonicalDocument(data []byte, maximum int, validator documentValidator) (map[string]any, error) {
	value, err := parseStrictJSON(data, maximum)
	if err != nil {
		return nil, err
	}
	document, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("document root must be an object")
	}
	if err := validator(document); err != nil {
		return nil, err
	}
	canonical, err := canonicalJSON(document)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(canonical, data) {
		return nil, fmt.Errorf("document is not exact canonical JSON")
	}
	return document, nil
}

func encodeValidated(document map[string]any, validator documentValidator) ([]byte, error) {
	if err := validator(document); err != nil {
		return nil, err
	}
	return canonicalJSON(document)
}
