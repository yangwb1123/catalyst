package gopackagedependencyobservationproducer

import (
	"bytes"
	"encoding/json"
	"fmt"
	"unicode/utf8"

	"forgeos/forge-core/internal/gopackagegraph"
)

// DecodeGraphObservation accepts the exact canonical graph document embedded
// by ADR-0062. It validates graph-internal derivations without claiming that a
// source manifest or current worktree was re-observed.
func DecodeGraphObservation(raw []byte) (gopackagegraph.Observation, string, error) {
	if len(raw) == 0 || len(raw) > maxProductionBytes || !utf8.Valid(raw) {
		return gopackagegraph.Observation{}, "", fmt.Errorf("graph JSON is empty, oversized, or invalid UTF-8")
	}
	if err := validateJSONShape(raw); err != nil {
		return gopackagegraph.Observation{}, "", fmt.Errorf("graph JSON shape: %w", err)
	}
	value, err := decodeGraphObservation(raw)
	if err != nil {
		return gopackagegraph.Observation{}, "", err
	}
	if err := validateStandaloneGraph(value); err != nil {
		return gopackagegraph.Observation{}, "", err
	}
	return gopackagegraph.CloneObservation(value), domainDigest(graphDigestDomain, raw), nil
}

func decodeGraphObservation(raw []byte) (gopackagegraph.Observation, error) {
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	var value gopackagegraph.Observation
	if err := decoder.Decode(&value); err != nil {
		return value, fmt.Errorf("decode graph JSON: %w", err)
	}
	if err := requireJSONEOF(decoder); err != nil {
		return value, err
	}
	canonical, err := canonicalJSON(value, maxProductionBytes)
	if err != nil || !bytes.Equal(canonical, raw) {
		return value, fmt.Errorf("graph JSON is not exact canonical encoding")
	}
	return value, nil
}

func validateStandaloneGraph(value gopackagegraph.Observation) error {
	if err := gopackagegraph.ValidateObservationSnapshot(value); err != nil {
		return fmt.Errorf("graph observation: %w", err)
	}
	producer := value.Producer
	if producer.ProducerID != ProducerID || producer.ProducerType != "tool" ||
		producer.ProducerVersion != ProducerVersion || !runIDPattern.MatchString(producer.RunID) {
		return fmt.Errorf("graph producer binding is invalid")
	}
	return nil
}
