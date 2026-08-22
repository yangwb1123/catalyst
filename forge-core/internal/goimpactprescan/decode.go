package goimpactprescan

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"unicode/utf8"
)

// Decode validates an exact ADR-0062 envelope by rebuilding its report from
// the embedded graph observation and changed paths.
func Decode(raw []byte) (*Production, error) {
	if len(raw) == 0 || len(raw) > maxEnvelopeBytes || !utf8.Valid(raw) {
		return nil, fmt.Errorf("impact envelope is empty, oversized, or invalid UTF-8")
	}
	if err := validateEnvelopeJSONShape(raw); err != nil {
		return nil, fmt.Errorf("impact envelope JSON shape: %w", err)
	}
	value, err := decodeEnvelope(raw)
	if err != nil {
		return nil, err
	}
	graphJSON, err := decodeEmbeddedGraph(value.Request.GraphObservationBase64URL)
	if err != nil {
		return nil, err
	}
	expected, err := Build(
		graphJSON, value.Request.GraphObservationSHA256,
		value.Request.RunID, value.Request.ChangedPaths,
	)
	if err != nil {
		return nil, fmt.Errorf("rebuild impact envelope: %w", err)
	}
	if !bytes.Equal(expected.JSON(), raw) {
		return nil, fmt.Errorf("impact envelope does not equal its deterministic reconstruction")
	}
	return expected, nil
}

func decodeEnvelope(raw []byte) (Envelope, error) {
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	var value Envelope
	if err := decoder.Decode(&value); err != nil {
		return value, fmt.Errorf("decode impact envelope: %w", err)
	}
	if _, err := decoder.Token(); err != io.EOF {
		return value, fmt.Errorf("impact envelope has trailing data")
	}
	canonical, err := canonicalJSON(value, maxEnvelopeBytes)
	if err != nil || !bytes.Equal(canonical, raw) {
		return value, fmt.Errorf("impact envelope is not exact canonical encoding")
	}
	return value, nil
}

func decodeEmbeddedGraph(value string) ([]byte, error) {
	if len(value) < 3 || len(value) > 22_369_622 {
		return nil, fmt.Errorf("embedded graph encoding violates bounds")
	}
	decoded, err := base64.RawURLEncoding.Strict().DecodeString(value)
	if err != nil || base64.RawURLEncoding.EncodeToString(decoded) != value {
		return nil, fmt.Errorf("embedded graph is not canonical unpadded Base64URL")
	}
	if len(decoded) == 0 || len(decoded) > maxGraphBytes {
		return nil, fmt.Errorf("embedded graph violates decoded bounds")
	}
	return decoded, nil
}
