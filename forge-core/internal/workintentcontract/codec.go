package workintentcontract

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
)

// DecodeCanonicalWorkIntent validates one exact canonical instance with no LF.
func DecodeCanonicalWorkIntent(data []byte) (*WorkIntent, error) {
	value, err := parseStrictJSON(data, maxRecordBytes)
	if err != nil {
		return nil, err
	}
	root, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("WorkIntent root must be an object")
	}
	if err := validateRawShape(root); err != nil {
		return nil, err
	}
	canonical, err := canonicalJSON(root)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(canonical, data) {
		return nil, fmt.Errorf("WorkIntent is not exact canonical JSON")
	}
	document, err := decodeTypedWorkIntent(data)
	if err != nil {
		return nil, err
	}
	if err := ValidateWorkIntent(document); err != nil {
		return nil, err
	}
	return document, nil
}

// CanonicalWorkIntentJSON validates and emits exact canonical instance bytes.
func CanonicalWorkIntentJSON(value *WorkIntent) ([]byte, error) {
	if err := ValidateWorkIntent(value); err != nil {
		return nil, err
	}
	root, err := workIntentNode(value)
	if err != nil {
		return nil, err
	}
	return canonicalJSON(root)
}

// WorkIntentSHA256 computes the digest after blanking only both identity fields.
func WorkIntentSHA256(value *WorkIntent) (string, error) {
	if value == nil {
		return "", fmt.Errorf("WorkIntent is nil")
	}
	blank := cloneWorkIntent(value)
	blank.WorkIntentID = ""
	blank.WorkIntentSHA256 = ""
	if err := validateWorkIntentFields(blank, true); err != nil {
		return "", err
	}
	root, err := workIntentNode(blank)
	if err != nil {
		return "", err
	}
	canonical, err := canonicalJSON(root)
	if err != nil {
		return "", fmt.Errorf("blank identity preimage: %w", err)
	}
	hasher := sha256.New()
	hasher.Write([]byte(digestDomain))
	hasher.Write(canonical)
	return hex.EncodeToString(hasher.Sum(nil)), nil
}

// SealWorkIntent returns a sealed deep copy of an exact blank-identity value.
func SealWorkIntent(value *WorkIntent) (*WorkIntent, error) {
	if value == nil || value.WorkIntentID != "" || value.WorkIntentSHA256 != "" {
		return nil, fmt.Errorf("sealing requires both identity fields to be empty")
	}
	if err := validateWorkIntentFields(value, true); err != nil {
		return nil, err
	}
	sealed := cloneWorkIntent(value)
	digest, err := WorkIntentSHA256(sealed)
	if err != nil {
		return nil, err
	}
	sealed.WorkIntentSHA256 = digest
	sealed.WorkIntentID = workIntentIDPrefix + digest
	if err := ValidateWorkIntent(sealed); err != nil {
		return nil, err
	}
	return sealed, nil
}

// ValidateWorkIntent verifies semantic shape, final bytes, digest, and ID.
func ValidateWorkIntent(value *WorkIntent) error {
	if err := validateWorkIntentFields(value, false); err != nil {
		return err
	}
	digest, err := WorkIntentSHA256(value)
	if err != nil {
		return err
	}
	if value.WorkIntentSHA256 != digest {
		return fmt.Errorf("work_intent_sha256 does not match the canonical preimage")
	}
	root, err := workIntentNode(value)
	if err != nil {
		return err
	}
	_, err = canonicalJSON(root)
	return err
}

func decodeTypedWorkIntent(data []byte) (*WorkIntent, error) {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var document WorkIntent
	if err := decoder.Decode(&document); err != nil {
		return nil, fmt.Errorf("typed WorkIntent decode: %w", err)
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		return nil, fmt.Errorf("typed WorkIntent has trailing data")
	}
	return &document, nil
}

func workIntentNode(value *WorkIntent) (map[string]any, error) {
	if value == nil {
		return nil, fmt.Errorf("WorkIntent is nil")
	}
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, err
	}
	raw := bytes.TrimSuffix(buffer.Bytes(), []byte{'\n'})
	parsed, err := parseStrictJSON(raw, maxTypedBytes)
	if err != nil {
		return nil, err
	}
	root, ok := parsed.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("typed WorkIntent did not encode as an object")
	}
	if err := validateRawShape(root); err != nil {
		return nil, err
	}
	return root, nil
}

func cloneWorkIntent(value *WorkIntent) *WorkIntent {
	copyValue := *value
	if value.DeclaredOwner != nil {
		owner := *value.DeclaredOwner
		copyValue.DeclaredOwner = &owner
	}
	if value.Binding.RunID != nil {
		runID := *value.Binding.RunID
		copyValue.Binding.RunID = &runID
	}
	if value.Origin.OriginRef != nil {
		originRef := *value.Origin.OriginRef
		copyValue.Origin.OriginRef = &originRef
	}
	copyValue.Intent = cloneIntent(value.Intent)
	copyValue.References = cloneReferences(value.References)
	return &copyValue
}

func cloneIntent(value Intent) Intent {
	result := value
	if value.DeadlineUnixMS != nil {
		deadline := *value.DeadlineUnixMS
		result.DeadlineUnixMS = &deadline
	}
	result.ExternalConstraints = cloneSlice(value.ExternalConstraints)
	result.NonGoals = cloneSlice(value.NonGoals)
	result.OpenQuestions = cloneSlice(value.OpenQuestions)
	result.Scope = cloneSlice(value.Scope)
	result.SuccessSignals = cloneSlice(value.SuccessSignals)
	return result
}

func cloneReferences(value References) References {
	result := value
	result.ClaimRecordRefs = cloneSlice(value.ClaimRecordRefs)
	result.EvidenceRecordRefs = cloneSlice(value.EvidenceRecordRefs)
	result.LocalArtifactDeclarations = cloneSlice(value.LocalArtifactDeclarations)
	if value.LocalSourceSnapshotDeclaration != nil {
		snapshot := *value.LocalSourceSnapshotDeclaration
		result.LocalSourceSnapshotDeclaration = &snapshot
	}
	return result
}

func cloneSlice[T any](value []T) []T {
	if value == nil {
		return nil
	}
	return append([]T{}, value...)
}
