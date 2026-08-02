package graphscheduledcontract

import (
	"bytes"
	"strings"
	"testing"
)

func TestDecodeCandidateAcceptsExactCanonicalSourceBoundValue(t *testing.T) {
	value := mustCandidate(t)
	encoded, err := MarshalCandidate(value)
	if err != nil {
		t.Fatalf("MarshalCandidate: %v", err)
	}
	decoded, err := DecodeCandidate(bytes.NewReader(encoded))
	if err != nil || ValidateCandidateSource(decoded, fixtureSnapshot(t)) != nil {
		t.Fatalf("decode/source validation: %v", err)
	}
	if !bytes.Equal(encoded, mustMarshal(t, decoded)) {
		t.Fatal("decoded candidate changed canonical bytes")
	}
}

func TestDecodeCandidateRejectsWireAndCanonicalDrift(t *testing.T) {
	canonical := string(mustMarshal(t, mustCandidate(t)))
	tests := []struct {
		name string
		data []byte
	}{
		{"empty", nil},
		{"duplicate", []byte(strings.Replace(canonical, `{"v":2,`, `{"v":2,"v":2,`, 1))},
		{"unknown", []byte(strings.Replace(canonical, `{"v":2,`, `{"v":2,"unknown":false,`, 1))},
		{"missing", []byte(strings.Replace(canonical, `"contract_scope":"schedule_initial_node_only",`, "", 1))},
		{"null array", []byte(strings.Replace(canonical, `"required_predecessor_node_ids":[]`, `"required_predecessor_node_ids":null`, 1))},
		{"reordered", []byte(strings.Replace(canonical, `{"v":2,"scheduler_protocol_version":1`, `{"scheduler_protocol_version":1,"v":2`, 1))},
		{"trailing whitespace", []byte(canonical + "\n")},
		{"trailing value", []byte(canonical + `{}`)},
		{"escaped html", []byte(strings.Replace(canonical, "<safely>", `\u003csafely>`, 1))},
		{"nested unknown", []byte(strings.Replace(canonical, `"execution_ordinal":0,`, `"execution_ordinal":0,"unknown":0,`, 1))},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if _, err := DecodeCandidate(bytes.NewReader(test.data)); err == nil {
				t.Fatal("DecodeCandidate accepted drift")
			}
		})
	}
}

func TestDecodeCandidateRejectsInvalidUTF8AndOversize(t *testing.T) {
	canonical := mustMarshal(t, mustCandidate(t))
	invalid := append([]byte(nil), canonical...)
	position := bytes.Index(invalid, []byte("graph-fixture-v1"))
	if position < 0 {
		t.Fatal("fixture identifier missing")
	}
	invalid[position] = 0xff
	for _, data := range [][]byte{invalid, bytes.Repeat([]byte{' '}, MaxCandidateBytes+1)} {
		if _, err := DecodeCandidate(bytes.NewReader(data)); err == nil {
			t.Fatal("DecodeCandidate accepted invalid bounded input")
		}
	}
}

func mustMarshal(t *testing.T, value ScheduledNodeContractCandidate) []byte {
	t.Helper()
	encoded, err := MarshalCandidate(value)
	if err != nil {
		t.Fatalf("MarshalCandidate: %v", err)
	}
	return encoded
}
