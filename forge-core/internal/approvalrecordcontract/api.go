package approvalrecordcontract

import (
	"bytes"
	"fmt"
)

type documentValidator func(map[string]any) error

// DecodeCanonicalRecord strictly decodes an authority-neutral ApprovalRecord v1.
func DecodeCanonicalRecord(data []byte) (map[string]any, error) {
	return decodeCanonicalDocument(data, maxRecordBytes, func(node map[string]any) error {
		return validateRecord(node, false)
	})
}

// DecodeCanonicalDeclaredTarget strictly decodes an ApprovalRecord target projection.
func DecodeCanonicalDeclaredTarget(data []byte) (map[string]any, error) {
	return decodeCanonicalDocument(data, maxTargetBytes, validateTarget)
}

// DecodeCanonicalAssessmentRequest strictly decodes a caller-time comparison request.
func DecodeCanonicalAssessmentRequest(data []byte) (map[string]any, error) {
	return decodeCanonicalDocument(data, maxRequestBytes, validateRequest)
}

// DecodeCanonicalAssessment strictly decodes a declared-only assessment.
func DecodeCanonicalAssessment(data []byte) (map[string]any, error) {
	return decodeCanonicalDocument(data, maxAssessmentBytes, validateAssessment)
}

// CanonicalRecordJSON validates and encodes an ApprovalRecord without mutation.
func CanonicalRecordJSON(record map[string]any) ([]byte, error) {
	return encodeValidated(record, func(node map[string]any) error { return validateRecord(node, false) })
}

// CanonicalDeclaredTargetJSON validates and encodes a declared target.
func CanonicalDeclaredTargetJSON(target map[string]any) ([]byte, error) {
	return encodeValidated(target, validateTarget)
}

// CanonicalAssessmentRequestJSON validates and encodes a declared assessment request.
func CanonicalAssessmentRequestJSON(request map[string]any) ([]byte, error) {
	return encodeValidated(request, validateRequest)
}

// CanonicalAssessmentJSON validates and encodes a declared-only assessment.
func CanonicalAssessmentJSON(assessment map[string]any) ([]byte, error) {
	return encodeValidated(assessment, validateAssessment)
}

// ApprovalRecordSHA256 returns the record's signature-independent digest.
func ApprovalRecordSHA256(record map[string]any) (string, error) {
	if err := validateRecord(record, true); err != nil {
		return "", err
	}
	return approvalDigest(record)
}

// DeclaredTarget projects every declaration compared by the pure assessment.
func DeclaredTarget(record map[string]any) (map[string]any, error) {
	return declaredTarget(record)
}

// DeclaredTargetSHA256 returns the frozen domain-separated target digest.
func DeclaredTargetSHA256(target map[string]any) (string, error) {
	return targetDigest(target)
}

// AssessDeclared compares only caller-declared data and grants no authority.
func AssessDeclared(request map[string]any) (map[string]any, error) {
	return assessDeclared(request)
}

// ValidateAssessment regenerates an assessment from the current request.
func ValidateAssessment(request, assessment map[string]any) error {
	if err := validateAssessment(assessment); err != nil {
		return err
	}
	return validateAssessmentAgainstRequest(request, assessment)
}

// ApprovalRef projects the exact reference shape consumed by CapabilityGrant v1.
func ApprovalRef(record map[string]any) (map[string]any, error) {
	if err := validateRecord(record, false); err != nil {
		return nil, err
	}
	proof := record["authority_proof"].(map[string]any)
	source := proof["authority_source"].(map[string]any)
	return map[string]any{"approval_id": record["approval_id"],
		"approval_sha256": record["approval_sha256"], "authority_domain": source["authority_domain"]}, nil
}

// ApprovalRefRelation checks only exact declared reference equality.
func ApprovalRefRelation(record, reference map[string]any) (string, error) {
	if err := validateApprovalRef(reference); err != nil {
		return "", err
	}
	expected, err := ApprovalRef(record)
	if err != nil {
		return "", err
	}
	if canonicalValuesEqual(reference, expected) {
		return "same_declared_reference", nil
	}
	return "reference_mismatch", nil
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
