package graphplan

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
)

func TestDecodeAcceptsCompleteStrictSpec(t *testing.T) {
	encoded, err := json.Marshal(validSpec())
	if err != nil {
		t.Fatalf("marshal fixture: %v", err)
	}
	got, err := Decode(bytes.NewReader(encoded))
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	if !equalSpec(got, validSpec()) {
		t.Fatalf("decoded spec differs: %#v", got)
	}
}

func TestDecodePreservesExplicitEmptyEdges(t *testing.T) {
	spec := validSpec()
	spec.Edges = []Edge{}
	encoded, err := json.Marshal(spec)
	if err != nil {
		t.Fatalf("marshal fanout: %v", err)
	}
	got, err := Decode(bytes.NewReader(encoded))
	if err != nil {
		t.Fatalf("Decode fanout: %v", err)
	}
	if got.Edges == nil {
		t.Fatal("explicit [] edges became null")
	}
}

func TestDecodeRejectsNonStrictInputWithoutDisclosure(t *testing.T) {
	tests := []struct {
		name string
		data []byte
	}{
		{"unknown top field", []byte(`{"v":1,"manager":{},"nodes":[],"edges":[],"TOP-SECRET-FIELD":1}`)},
		{"unknown nested field", []byte(`{"v":1,"manager":{"agent_profile":"m","instruction":"i","TOP-SECRET-FIELD":1},"nodes":[],"edges":[]}`)},
		{"case folded field", []byte(`{"V":1,"manager":{},"nodes":[],"edges":[]}`)},
		{"duplicate manager field", bytes.Replace(validSpecJSON(t), []byte(`"agent_profile":"integration-manager"`), []byte(`"agent_profile":"integration-manager","agent_profile":"TOP-SECRET-FIELD"`), 1)},
		{"duplicate node field", bytes.Replace(validSpecJSON(t), []byte(`"node_id":"frontend"`), []byte(`"node_id":"frontend","node_id":"TOP-SECRET-FIELD"`), 1)},
		{"duplicate edge field", bytes.Replace(validSpecJSON(t), []byte(`"from_node_id":"frontend"`), []byte(`"from_node_id":"frontend","from_node_id":"TOP-SECRET-FIELD"`), 1)},
		{"trailing JSON", append(validSpecJSON(t), []byte(`{"TOP-SECRET-FIELD":1}`)...)},
		{"invalid UTF-8", append(validSpecJSON(t), 0xff)},
		{"invalid UTF-8 inside field", invalidUTF8InsideField(t)},
		{"lone high surrogate", bytes.Replace(validSpecJSON(t), []byte("Coordinate frontend, backend, and SSO."), []byte(`\ud800`), 1)},
		{"lone low surrogate", bytes.Replace(validSpecJSON(t), []byte("Coordinate frontend, backend, and SSO."), []byte(`\udc00`), 1)},
		{"missing edges", []byte(`{"v":1,"manager":{"agent_profile":"m","instruction":"i"},"nodes":[]}`)},
		{"null edges", []byte(`{"v":1,"manager":{"agent_profile":"m","instruction":"i"},"nodes":[],"edges":null}`)},
		{"duplicate field", []byte(`{"v":1,"v":1,"manager":{},"nodes":[],"edges":[]}`)},
		{"unsupported version", []byte(`{"v":2,"manager":{},"nodes":[],"edges":[]}`)},
		{"malformed", []byte(`{"v":"TOP-SECRET-VALUE"`)},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			_, err := Decode(bytes.NewReader(test.data))
			if err == nil {
				t.Fatal("Decode accepted invalid input")
			}
			if strings.Contains(err.Error(), "TOP-SECRET") {
				t.Fatalf("error disclosed input content: %v", err)
			}
		})
	}
}

func TestDecodeRejectsOversizeInput(t *testing.T) {
	oversize := bytes.Repeat([]byte{' '}, MaxSpecBytes+1)
	if _, err := Decode(bytes.NewReader(oversize)); err == nil {
		t.Fatal("Decode accepted an oversized spec")
	}
}

func TestDecodeAcceptsPairedUnicodeSurrogate(t *testing.T) {
	encoded := bytes.Replace(
		validSpecJSON(t),
		[]byte("Coordinate frontend, backend, and SSO."),
		[]byte(`\ud83d\ude00`),
		1,
	)
	if _, err := Decode(bytes.NewReader(encoded)); err != nil {
		t.Fatalf("Decode valid surrogate pair: %v", err)
	}
}

func TestBuildRejectsInvalidFieldsAndBounds(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*Spec)
	}{
		{"zero nodes", func(spec *Spec) { spec.Nodes = []Node{} }},
		{"too many nodes", func(spec *Spec) {
			for len(spec.Nodes) <= maxNodes {
				spec.Nodes = append(spec.Nodes, Node{
					NodeID:    "extra-" + strings.Repeat("x", len(spec.Nodes)),
					ProjectID: "project", MemberRole: "role", AgentProfile: "agent",
					Task: "task", Acceptance: "acceptance",
				})
			}
			spec.Edges = []Edge{}
		}},
		{"too many edges", func(spec *Spec) {
			spec.Edges = make([]Edge, maxEdges+1)
		}},
		{"identifier control", func(spec *Spec) { spec.Nodes[0].NodeID = "front\nend" }},
		{"identifier bidi", func(spec *Spec) { spec.Nodes[0].NodeID = "front\u202eend" }},
		{"identifier invalid UTF-8", func(spec *Spec) {
			spec.Nodes[0].NodeID = string([]byte{'f', 0xff})
		}},
		{"empty manager", func(spec *Spec) { spec.Manager.Instruction = " \t " }},
		{"prose control", func(spec *Spec) { spec.Nodes[0].Task = "task\x00secret" }},
		{"prose invalid UTF-8", func(spec *Spec) {
			spec.Nodes[0].Task = string([]byte{'t', 0xff})
		}},
		{"oversized prose", func(spec *Spec) {
			spec.Nodes[0].Task = strings.Repeat("x", maxProseBytes+1)
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			spec := validSpec()
			test.mutate(&spec)
			if _, err := Build(spec, "graph", fixtureManifestSHA); err == nil {
				t.Fatal("Build accepted invalid fields")
			}
		})
	}
}

func validSpecJSON(t *testing.T) []byte {
	t.Helper()
	encoded, err := json.Marshal(validSpec())
	if err != nil {
		t.Fatalf("marshal fixture: %v", err)
	}
	return encoded
}

func invalidUTF8InsideField(t *testing.T) []byte {
	t.Helper()
	return bytes.Replace(
		validSpecJSON(t),
		[]byte("integration-manager"),
		[]byte{'i', 'n', 0xff, 'v'},
		1,
	)
}

func equalSpec(left, right Spec) bool {
	leftJSON, leftErr := json.Marshal(left)
	rightJSON, rightErr := json.Marshal(right)
	return leftErr == nil && rightErr == nil && bytes.Equal(leftJSON, rightJSON)
}
