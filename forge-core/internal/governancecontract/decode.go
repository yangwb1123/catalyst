package governancecontract

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
)

const structuralValidity = "STRUCTURALLY_VALID (shadow; no truth or authority attestation)"

func DecodeRecord(data []byte) (*Record, error) {
	node, err := parseStrictJSON(data)
	if err != nil {
		return nil, fmt.Errorf("governance record JSON: %w", err)
	}
	root, ok := node.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("governance record root must be an object")
	}
	kind, ok := root["kind"].(string)
	if !ok {
		return nil, fmt.Errorf("governance record kind must be a string")
	}
	if err := validateRecordShape(root, kind); err != nil {
		return nil, err
	}
	canonicalRecord, err := canonicalJSON(root)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(data, canonicalRecord) {
		return nil, fmt.Errorf("record input is not exact compact canonical JSON")
	}
	record, err := decodeTypedRecord(data, kind)
	if err != nil {
		return nil, err
	}
	if err := requireTypedWireEquivalent(record, canonicalRecord); err != nil {
		return nil, err
	}
	record.node, record.canonicalRecord = root, canonicalRecord
	if err := finishDigest(record); err != nil {
		return nil, err
	}
	return record, nil
}

func requireTypedWireEquivalent(record *Record, canonicalRecord []byte) error {
	state, err := canonicalStateFromTyped(record)
	if err != nil {
		return fmt.Errorf("typed governance record: %w", err)
	}
	if !bytes.Equal(state.record, canonicalRecord) {
		return fmt.Errorf("typed governance record does not preserve exact canonical input")
	}
	return nil
}

func decodeTypedRecord(data []byte, kind string) (*Record, error) {
	switch kind {
	case EvidenceKind:
		var value EvidenceRecord
		if err := decodeStrictTyped(data, &value); err != nil {
			return nil, fmt.Errorf("EvidenceRecord: %w", err)
		}
		if err := validateEvidence(&value); err != nil {
			return nil, fmt.Errorf("EvidenceRecord: %w", err)
		}
		return &Record{Evidence: &value}, nil
	case ClaimKind:
		var value KnowledgeClaim
		if err := decodeStrictTyped(data, &value); err != nil {
			return nil, fmt.Errorf("KnowledgeClaim: %w", err)
		}
		if err := validateClaim(&value); err != nil {
			return nil, fmt.Errorf("KnowledgeClaim: %w", err)
		}
		return &Record{Claim: &value}, nil
	default:
		return nil, fmt.Errorf("unsupported governance record kind %q", kind)
	}
}

func decodeStrictTyped(data []byte, target any) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	decoder.UseNumber()
	if err := decoder.Decode(target); err != nil {
		return err
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		return fmt.Errorf("unexpected trailing JSON value")
	}
	return nil
}

func finishDigest(record *Record) error {
	state, err := canonicalStateFromNode(record.node, record.Kind())
	if err != nil {
		return err
	}
	if storedDigest(record) != state.digest {
		return fmt.Errorf("canonical_sha256 mismatch: got %q want %q", storedDigest(record), state.digest)
	}
	applyCanonicalState(record, state)
	return nil
}

type canonicalState struct {
	node    any
	record  []byte
	payload []byte
	digest  string
}

func canonicalStateFromNode(node any, kind string) (canonicalState, error) {
	recordJSON, err := canonicalJSON(node)
	if err != nil {
		return canonicalState{}, err
	}
	if len(recordJSON) > maxRecordBytes {
		return canonicalState{}, fmt.Errorf("canonical record exceeds %d bytes", maxRecordBytes)
	}
	payloadNode := cloneNode(node).(map[string]any)
	payloadNode["integrity"].(map[string]any)["canonical_sha256"] = ""
	payload, err := canonicalJSON(payloadNode)
	if err != nil {
		return canonicalState{}, err
	}
	if len(payload)+sha256.Size*2 > maxRecordBytes {
		return canonicalState{}, fmt.Errorf("canonical sealed record exceeds %d bytes", maxRecordBytes)
	}
	hasher := sha256.New()
	hasher.Write([]byte(digestDomain(kind)))
	hasher.Write([]byte{0})
	hasher.Write(payload)
	digest := hex.EncodeToString(hasher.Sum(nil))
	return canonicalState{node: node, record: recordJSON, payload: payload, digest: digest}, nil
}

func canonicalStateFromTyped(record *Record) (canonicalState, error) {
	data, err := marshalTypedRecord(record)
	if err != nil {
		return canonicalState{}, err
	}
	node, err := parseStrictJSONBounded(data, len(data))
	if err != nil {
		return canonicalState{}, err
	}
	root, ok := node.(map[string]any)
	if !ok {
		return canonicalState{}, fmt.Errorf("typed record did not encode as an object")
	}
	if err := validateRecordShape(root, record.Kind()); err != nil {
		return canonicalState{}, err
	}
	return canonicalStateFromNode(root, record.Kind())
}

func marshalTypedRecord(record *Record) ([]byte, error) {
	if record.Evidence != nil {
		return encodeTypedJSON(record.Evidence)
	}
	if record.Claim != nil {
		return encodeTypedJSON(record.Claim)
	}
	return nil, fmt.Errorf("record union has no typed value")
}

func verifiedCanonicalState(record *Record) (canonicalState, error) {
	if record == nil || record.Header() == nil || (record.Evidence != nil) == (record.Claim != nil) {
		return canonicalState{}, fmt.Errorf("record union must contain exactly one kind")
	}
	var err error
	if record.Evidence != nil {
		err = validateEvidence(record.Evidence)
	} else {
		err = validateClaim(record.Claim)
	}
	if err != nil {
		return canonicalState{}, err
	}
	state, err := canonicalStateFromTyped(record)
	if err != nil {
		return canonicalState{}, err
	}
	if storedDigest(record) != state.digest {
		return canonicalState{}, fmt.Errorf("canonical_sha256 mismatch: got %q want %q", storedDigest(record), state.digest)
	}
	return state, nil
}

func applyCanonicalState(record *Record, state canonicalState) {
	record.node = state.node
	record.canonicalRecord = state.record
	record.canonicalPayload = state.payload
	record.digest = state.digest
}

func digestDomain(kind string) string {
	if kind == EvidenceKind {
		return "forgeos.governance.evidence-record.v1"
	}
	return "forgeos.governance.knowledge-claim.v1"
}

func storedDigest(record *Record) string {
	if record.Evidence != nil {
		return record.Evidence.Integrity.CanonicalSHA256
	}
	return record.Claim.Integrity.CanonicalSHA256
}
