package knowledgeupdateproposalcontract

import (
	"bytes"
	"fmt"
)

type documentValidator func(map[string]any) error

// DecodeCanonicalProposal strictly decodes a contract-only proposal.
func DecodeCanonicalProposal(data []byte) (map[string]any, error) {
	return decodeCanonicalDocument(data, maxProposalBytes, validateProposal)
}

// DecodeCanonicalDeclaredTarget strictly decodes a declared target projection.
func DecodeCanonicalDeclaredTarget(data []byte) (map[string]any, error) {
	return decodeCanonicalDocument(data, maxTargetBytes, validateTarget)
}

// DecodeCanonicalAssessmentRequest strictly decodes a pure comparison request.
func DecodeCanonicalAssessmentRequest(data []byte) (map[string]any, error) {
	return decodeCanonicalDocument(data, maxRequestBytes, validateRequest)
}

// DecodeCanonicalAssessment strictly decodes an authority-neutral assessment.
func DecodeCanonicalAssessment(data []byte) (map[string]any, error) {
	return decodeCanonicalDocument(data, maxAssessmentBytes, validateAssessment)
}

// CanonicalProposalJSON validates and encodes a proposal without mutation.
func CanonicalProposalJSON(value map[string]any) ([]byte, error) {
	return encodeValidated(value, validateProposal)
}

// CanonicalDeclaredTargetJSON validates and encodes a target without mutation.
func CanonicalDeclaredTargetJSON(value map[string]any) ([]byte, error) {
	return encodeValidated(value, validateTarget)
}

// CanonicalAssessmentRequestJSON validates and encodes a request without mutation.
func CanonicalAssessmentRequestJSON(value map[string]any) ([]byte, error) {
	return encodeValidated(value, validateRequest)
}

// CanonicalAssessmentJSON validates and encodes an assessment without mutation.
func CanonicalAssessmentJSON(value map[string]any) ([]byte, error) {
	return encodeValidated(value, validateAssessment)
}

// RecordSetSHA256 validates a proposal record set and returns its frozen digest.
func RecordSetSHA256(records []any) (string, error) {
	if _, err := validateRecordSet(records); err != nil {
		return "", err
	}
	return digestValue(recordSetDomain, records)
}

// KnowledgeUpdateProposalSHA256 returns the proposal's self-identity digest.
func KnowledgeUpdateProposalSHA256(value map[string]any) (string, error) {
	if err := validateProposal(value); err != nil {
		return "", err
	}
	return proposalDigest(value)
}

// DeclaredTarget projects every proposal declaration compared by the evaluator.
func DeclaredTarget(value map[string]any) (map[string]any, error) {
	return declaredTarget(value)
}

// DeclaredTargetSHA256 returns the target's domain-separated digest.
func DeclaredTargetSHA256(value map[string]any) (string, error) {
	return targetDigest(value)
}

// AssessmentRequestSHA256 returns the request's domain-separated digest.
func AssessmentRequestSHA256(value map[string]any) (string, error) {
	if err := validateRequest(value); err != nil {
		return "", err
	}
	return requestDigest(value)
}

// AssessmentSHA256 returns the assessment's domain-separated self digest.
func AssessmentSHA256(value map[string]any) (string, error) {
	if err := validateAssessment(value); err != nil {
		return "", err
	}
	return assessmentDigest(value)
}

// AssessDeclared compares caller declarations only and changes no state.
func AssessDeclared(request map[string]any) (map[string]any, error) {
	return assessDeclared(request)
}

// ValidateAssessment regenerates an assessment from its explicit request.
func ValidateAssessment(request, assessment map[string]any) error {
	if err := validateAssessment(assessment); err != nil {
		return err
	}
	expected, err := assessDeclared(request)
	if err != nil {
		return err
	}
	if !canonicalValuesEqual(expected, assessment) {
		return fmt.Errorf("assessment differs from fresh authority-neutral declared assessment")
	}
	return nil
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
