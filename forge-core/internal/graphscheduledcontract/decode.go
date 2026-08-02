package graphscheduledcontract

import (
	"bytes"
	"encoding/json"
	"io"
	"unicode/utf8"
)

// DecodeCandidate reads one bounded, strict, exactly canonical candidate.
// Source-bound callers must additionally call ValidateCandidateSource.
func DecodeCandidate(reader io.Reader) (ScheduledNodeContractCandidate, error) {
	data, err := io.ReadAll(io.LimitReader(reader, MaxCandidateBytes+1))
	if err != nil || len(data) == 0 || len(data) > MaxCandidateBytes ||
		!utf8.Valid(data) || rejectDuplicateFields(data) != nil {
		return ScheduledNodeContractCandidate{}, errInvalidCandidate
	}
	value, err := decodeExact[ScheduledNodeContractCandidate](data)
	if err != nil || validateCandidate(value) != nil {
		return ScheduledNodeContractCandidate{}, errInvalidCandidate
	}
	return value, nil
}

func decodeExact[T any](data []byte) (T, error) {
	var value T
	if len(data) == 0 || !utf8.Valid(data) || rejectDuplicateFields(data) != nil {
		return value, errInvalidCandidate
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&value); err != nil {
		return value, errInvalidCandidate
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return value, errInvalidCandidate
	}
	canonical, err := canonicalBytes(value)
	if err != nil || !bytes.Equal(data, canonical) {
		return value, errInvalidCandidate
	}
	return value, nil
}
