package projectsnapshot

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
)

// Decode accepts only one exact compact canonical v1 production and rederives
// every semantic binding and digest before returning a defensive value.
func Decode(raw []byte) (*Production, error) {
	if len(raw) == 0 || len(raw) > maxEnvelopeBytes {
		return nil, fmt.Errorf("project snapshot production bytes are outside bounds")
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	var envelope Envelope
	if err := decoder.Decode(&envelope); err != nil {
		return nil, fmt.Errorf("decode project snapshot production: %w", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return nil, fmt.Errorf("project snapshot production has trailing data")
	}
	production := &Production{
		envelope: cloneEnvelope(envelope), encoded: append([]byte(nil), raw...),
	}
	if err := Validate(production); err != nil {
		return nil, fmt.Errorf("validate project snapshot production: %w", err)
	}
	return production, nil
}
