package platformcorecontract

import (
	"bytes"
	"testing"
)

func TestCommandStrictDecoderRejectsMalformedWire(t *testing.T) {
	fixture := loadGoldenFixture(t)
	canonical, err := CanonicalCommandEnvelopeJSON(&fixture.CommandEnvelope)
	if err != nil {
		t.Fatal(err)
	}
	cases := map[string][]byte{
		"leading whitespace": append([]byte(" "), canonical...),
		"trailing value":     append(append([]byte(nil), canonical...), []byte("null")...),
		"unknown root":       bytes.Replace(canonical, []byte(`{"actor_ref":`), []byte(`{"extra":true,"actor_ref":`), 1),
		"duplicate root":     bytes.Replace(canonical, []byte(`{"actor_ref":`), []byte(`{"actor_ref":{"actor_id":"acr_0000000000000000000000000a","actor_type":"service"},"actor_ref":`), 1),
		"duplicate payload":  bytes.Replace(canonical, []byte(`"payload":{"agent_adapter":"codex",`), []byte(`"payload":{"agent_adapter":"codex","agent_adapter":"codex",`), 1),
		"float":              bytes.Replace(canonical, []byte(`"expected_version":3`), []byte(`"expected_version":3.0`), 1),
		"overflow":           bytes.Replace(canonical, []byte(`"expected_version":3`), []byte(`"expected_version":9223372036854775808`), 1),
		"boolean envelope":   bytes.Replace(canonical, []byte(`"envelope_version":1`), []byte(`"envelope_version":true`), 1),
		"boolean schema":     bytes.Replace(canonical, []byte(`"schema_version":1`), []byte(`"schema_version":true`), 1),
		"boolean expected":   bytes.Replace(canonical, []byte(`"expected_version":3`), []byte(`"expected_version":true`), 1),
		"boolean issued":     bytes.Replace(canonical, []byte(`"issued_at_unix_ms":1787961600000`), []byte(`"issued_at_unix_ms":true`), 1),
		"boolean deadline":   bytes.Replace(canonical, []byte(`"deadline_unix_ms":1787961660000`), []byte(`"deadline_unix_ms":true`), 1),
		"future envelope":    bytes.Replace(canonical, []byte(`"envelope_version":1`), []byte(`"envelope_version":2`), 1),
		"missing nullable":   bytes.Replace(canonical, []byte(`"causation_id":null,`), nil, 1),
		"forbidden Unicode":  bytes.Replace(canonical, []byte(`"agent_adapter":"codex"`), []byte(`"agent_adapter":"codex\u202e"`), 1),
		"invalid UTF-8":      append(append([]byte(nil), canonical[:20]...), append([]byte{0xff}, canonical[21:]...)...),
	}
	for name, raw := range cases {
		t.Run(name, func(t *testing.T) {
			if _, err := DecodeCanonicalCommandEnvelope(raw); err == nil {
				t.Fatal("expected strict rejection")
			}
		})
	}
}

func TestEventStrictDecoderRejectsNestedUnknownAndMissingFields(t *testing.T) {
	fixture := loadGoldenFixture(t)
	canonical, err := CanonicalEventEnvelopeJSON(&fixture.EventEnvelope)
	if err != nil {
		t.Fatal(err)
	}
	cases := [][]byte{
		bytes.Replace(canonical, []byte(`"actor_type":"service"`), []byte(`"actor_type":"service","extra":true`), 1),
		bytes.Replace(canonical, []byte(`"source_snapshot_ref":{"entity_id"`), []byte(`"source_snapshot_ref":{"extra":true,"entity_id"`), 1),
		bytes.Replace(canonical, []byte(`"extensions":{},`), nil, 1),
		bytes.Replace(canonical, []byte(`"envelope_version":1`), []byte(`"envelope_version":2`), 1),
		bytes.Replace(canonical, []byte(`"aggregate_version":4`), []byte(`"aggregate_version":true`), 1),
		bytes.Replace(canonical, []byte(`"occurred_at_unix_ms":1787961605000`), []byte(`"occurred_at_unix_ms":true`), 1),
		bytes.Replace(canonical, []byte(`"sequence":9`), []byte(`"sequence":true`), 1),
	}
	for index, raw := range cases {
		if _, err := DecodeCanonicalEventEnvelope(raw); err == nil {
			t.Fatalf("case %d: expected rejection", index)
		}
	}
}

func TestArtifactStrictDecoderRejectsIdentityAndFramingDrift(t *testing.T) {
	fixture := loadGoldenFixture(t)
	canonical, err := CanonicalArtifactRefJSON(&fixture.ArtifactRef)
	if err != nil {
		t.Fatal(err)
	}
	cases := [][]byte{
		bytes.Replace(canonical, []byte(`"size_bytes":4096`), []byte(`"size_bytes":4e3`), 1),
		bytes.Replace(canonical, []byte(`"size_bytes":4096`), []byte(`"size_bytes":true`), 1),
		bytes.Replace(canonical, []byte(`"created_at_unix_ms":1787961604000`), []byte(`"created_at_unix_ms":true`), 1),
		bytes.Replace(canonical, []byte(`"content_digest":`), []byte(`"extra":null,"content_digest":`), 1),
		bytes.Replace(canonical, []byte(`"sensitivity":"internal",`), nil, 1),
		append(append([]byte(nil), canonical...), '\n'),
	}
	for index, raw := range cases {
		if _, err := DecodeCanonicalArtifactRef(raw); err == nil {
			t.Fatalf("case %d: expected rejection", index)
		}
	}
}

func TestValidateReferenceAlwaysReturnsCodedErrors(t *testing.T) {
	fixture := loadGoldenFixture(t)
	reference := fixture.ArtifactRef.ProvenanceRef
	reference.RecordType = "invalid"
	err := ValidateReference(reference)
	if _, ok := RejectionCodeOf(err); !ok {
		t.Fatalf("ValidateReference returned uncoded error: %v", err)
	}
}
