package graphscheduledreconcile

import (
	"bytes"
	"encoding/json"
	"io"
	"unicode/utf8"
)

// DecodeSnapshot reads one bounded, strict, exact canonical progress snapshot.
func DecodeSnapshot(reader io.Reader) (ProgressSnapshot, error) {
	data, err := io.ReadAll(io.LimitReader(reader, MaxProgressSnapshotBytes+1))
	if err != nil || len(data) == 0 || len(data) > MaxProgressSnapshotBytes ||
		!utf8.Valid(data) || rejectDuplicateFields(data) != nil {
		return ProgressSnapshot{}, errInvalidSnapshot
	}
	snapshot, err := decodeSnapshotWire(data)
	if err != nil || validateSnapshot(snapshot) != nil {
		return ProgressSnapshot{}, errInvalidSnapshot
	}
	canonical, err := canonicalBytes(snapshot)
	if err != nil || !bytes.Equal(data, canonical) {
		return ProgressSnapshot{}, errInvalidSnapshot
	}
	return snapshot, nil
}

func decodeSnapshotWire(data []byte) (ProgressSnapshot, error) {
	var snapshot ProgressSnapshot
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&snapshot); err != nil {
		return ProgressSnapshot{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return ProgressSnapshot{}, errInvalidSnapshot
	}
	return snapshot, nil
}
