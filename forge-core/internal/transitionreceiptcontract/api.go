package transitionreceiptcontract

import (
	"bytes"
	"fmt"
)

type documentValidator func(map[string]any) error

// DecodeCanonicalVocabulary strictly decodes the frozen 23-state vocabulary.
func DecodeCanonicalVocabulary(data []byte) (map[string]any, error) {
	return decodeCanonicalDocument(data, maxVocabularyBytes, validateVocabulary)
}

// DecodeCanonicalReceipt strictly decodes an authority-neutral receipt.
func DecodeCanonicalReceipt(data []byte) (map[string]any, error) {
	return decodeCanonicalDocument(data, maxReceiptBytes, func(node map[string]any) error {
		return validateReceipt(node, false)
	})
}

// DecodeCanonicalDeclaredTarget strictly decodes a declared target.
func DecodeCanonicalDeclaredTarget(data []byte) (map[string]any, error) {
	return decodeCanonicalDocument(data, maxTargetBytes, validateTarget)
}

// DecodeCanonicalAssessmentRequest decodes an explicit pure comparison request.
func DecodeCanonicalAssessmentRequest(data []byte) (map[string]any, error) {
	return decodeCanonicalDocument(data, maxRequestBytes, validateRequest)
}

// DecodeCanonicalAssessment decodes an authority-neutral assessment.
func DecodeCanonicalAssessment(data []byte) (map[string]any, error) {
	return decodeCanonicalDocument(data, maxAssessmentBytes, validateAssessment)
}

// CanonicalVocabularyJSON validates and encodes a vocabulary without mutation.
func CanonicalVocabularyJSON(value map[string]any) ([]byte, error) {
	return encodeValidated(value, validateVocabulary)
}

// CanonicalReceiptJSON validates and encodes a receipt without mutation.
func CanonicalReceiptJSON(value map[string]any) ([]byte, error) {
	return encodeValidated(value, func(node map[string]any) error { return validateReceipt(node, false) })
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

// TransitionVocabulary returns an independent copy of the exact authored graph.
func TransitionVocabulary() (map[string]any, error) {
	return authoredVocabulary()
}

// TransitionVocabularySHA256 returns the graph's domain-separated digest.
func TransitionVocabularySHA256(value map[string]any) (string, error) {
	if err := validateVocabulary(value); err != nil {
		return "", err
	}
	return vocabularyDigest(value)
}

// TransitionReceiptSHA256 returns the receipt's self-identity digest.
func TransitionReceiptSHA256(value map[string]any) (string, error) {
	if err := validateReceipt(value, true); err != nil {
		return "", err
	}
	return receiptDigest(value)
}

// DeclaredTarget projects every declaration compared by the pure evaluator.
func DeclaredTarget(value map[string]any) (map[string]any, error) {
	return declaredTarget(value)
}

// DeclaredTargetSHA256 returns the target's domain-separated digest.
func DeclaredTargetSHA256(value map[string]any) (string, error) {
	return targetDigest(value)
}

// AssessDeclared compares only caller-declared data and changes no state.
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

func decodeCanonicalDocument(data []byte, maximum int,
	validator documentValidator) (map[string]any, error) {
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
