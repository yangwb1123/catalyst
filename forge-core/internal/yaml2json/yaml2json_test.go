package yaml2json

import (
	"bytes"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

// ── Basic scalar tests ────────────────────────────────────────────────────

func TestScalar_String(t *testing.T) {
	tests := []struct {
		input string
		want  any
	}{
		{"hello", "hello"},
		{"hello world", "hello world"},
		{"'quoted'", "quoted"},
		{"\"double\"", "double"},
		{"123", float64(123)},
		{"3.14", 3.14},
		{"true", true},
		{"false", false},
		{"null", nil},
		{"~", nil},
		{"key: value", "key: value"}, // not a mapping when standalone
	}
	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			got := parseScalar(tt.input)
			if got != tt.want {
				t.Errorf("parseScalar(%q) = %v (%T), want %v (%T)", tt.input, got, got, tt.want, tt.want)
			}
		})
	}
}

func TestScalar_Numbers(t *testing.T) {
	if v := parseScalar("42"); v != float64(42) {
		t.Errorf("parseScalar(42) = %v (%T), want float64(42)", v, v)
	}
	if v := parseScalar("3.14"); v != 3.14 {
		t.Errorf("parseScalar(3.14) = %v (%T), want 3.14", v, v)
	}
	if v := parseScalar("-5"); v != float64(-5) {
		t.Errorf("parseScalar(-5) = %v (%T), want float64(-5)", v, v)
	}
	if v := parseScalar("1e10"); v != 1e10 {
		t.Errorf("parseScalar(1e10) = %v (%T), want 1e10", v, v)
	}
}

// ── Simple mapping tests ──────────────────────────────────────────────────

func TestDecode_SimpleMapping(t *testing.T) {
	input := "key: value\nnum: 42\nflag: true\n"
	val, err := Decode(strings.NewReader(input))
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	m, ok := val.(map[string]any)
	if !ok {
		t.Fatalf("expected map, got %T", val)
	}
	if m["key"] != "value" {
		t.Errorf("key = %v, want value", m["key"])
	}
	if m["num"] != float64(42) {
		t.Errorf("num = %v, want 42", m["num"])
	}
	if m["flag"] != true {
		t.Errorf("flag = %v, want true", m["flag"])
	}
}

func TestDecode_NestedMapping(t *testing.T) {
	input := "outer:\n  inner: value\n  num: 99\n"
	val, err := Decode(strings.NewReader(input))
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	m := val.(map[string]any)
	inner := m["outer"].(map[string]any)
	if inner["inner"] != "value" {
		t.Errorf("inner.inner = %v, want value", inner["inner"])
	}
	if inner["num"] != float64(99) {
		t.Errorf("inner.num = %v, want 99", inner["num"])
	}
}

// ── Sequence tests ────────────────────────────────────────────────────────

func TestDecode_SimpleSequence(t *testing.T) {
	input := "- one\n- two\n- three\n"
	val, err := Decode(strings.NewReader(input))
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	seq, ok := val.([]any)
	if !ok {
		t.Fatalf("expected []any, got %T", val)
	}
	if len(seq) != 3 || seq[0] != "one" || seq[1] != "two" || seq[2] != "three" {
		t.Errorf("seq = %v, want [one two three]", seq)
	}
}

func TestDecode_SequenceOfMappings(t *testing.T) {
	input := `- name: first
  value: 1
- name: second
  value: 2
`
	val, err := Decode(strings.NewReader(input))
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	seq, ok := val.([]any)
	if !ok {
		t.Fatalf("expected []any, got %T", val)
	}
	if len(seq) != 2 {
		t.Fatalf("len = %d, want 2", len(seq))
	}
	first, ok := seq[0].(map[string]any)
	if !ok || first["name"] != "first" || first["value"] != float64(1) {
		t.Errorf("first = %v, want {name:first value:1}", seq[0])
	}
	second, ok := seq[1].(map[string]any)
	if !ok || second["name"] != "second" || second["value"] != float64(2) {
		t.Errorf("second = %v, want {name:second value:2}", seq[1])
	}
}

// TestDecode_SequenceBareDashNull asserts that a bare "-" sequence item (no
// inline value, i.e. YAML null) decodes identically to a spelled-out "null"
// item in the same position, and that both are appended — matching PyYAML's
// "a: [null, null]" decoding to a 2-element array, not 1. This guards
// against a regression of the empty-item branch silently dropping bare-dash
// null items instead of appending a Go nil like every other branch does.
func TestDecode_SequenceBareDashNull(t *testing.T) {
	input := "a:\n  -\n  - null\n"
	val, err := Decode(strings.NewReader(input))
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	m, ok := val.(map[string]any)
	if !ok {
		t.Fatalf("expected map[string]any, got %T", val)
	}
	seq, ok := m["a"].([]any)
	if !ok {
		t.Fatalf("expected []any for key a, got %T", m["a"])
	}
	if len(seq) != 2 {
		t.Fatalf("len(seq) = %d, want 2 (bare '-' item was dropped)", len(seq))
	}
	if seq[0] != nil {
		t.Errorf("seq[0] = %v (%T), want nil (bare '-' item)", seq[0], seq[0])
	}
	if seq[1] != nil {
		t.Errorf("seq[1] = %v (%T), want nil (spelled-out 'null' item)", seq[1], seq[1])
	}
}

// ── Inline sequence tests ─────────────────────────────────────────────────

func TestDecode_InlineSequence(t *testing.T) {
	input := "gates: [lint, test, build]\n"
	val, err := Decode(strings.NewReader(input))
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	m := val.(map[string]any)
	seq, ok := m["gates"].([]any)
	if !ok || len(seq) != 3 || seq[0] != "lint" {
		t.Errorf("gates = %v, want [lint test build]", m["gates"])
	}
}

// ── Comment handling ──────────────────────────────────────────────────────

func TestDecode_Comments(t *testing.T) {
	input := "key: value # inline comment\n# full comment\nnext: 42\n"
	val, err := Decode(strings.NewReader(input))
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	m := val.(map[string]any)
	if m["key"] != "value" || m["next"] != float64(42) {
		t.Errorf("m = %v, want {key:value next:42}", m)
	}
}

// ── Empty / null handling ─────────────────────────────────────────────────

func TestDecode_Empty(t *testing.T) {
	val, err := Decode(strings.NewReader(""))
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	if val != nil {
		t.Errorf("expected nil, got %v", val)
	}
}

// ── modes.yml integration test ────────────────────────────────────────────

// repoRoot finds the repo root by looking for forge-core/go.mod.
func repoRoot(t *testing.T) string {
	t.Helper()
	dir, err := os.Getwd()
	if err != nil {
		t.Skip("cannot get cwd")
	}
	for {
		if _, err := os.Stat(filepath.Join(dir, "forge-core", "go.mod")); err == nil {
			return dir
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			t.Skip("not inside ForgeOS repo")
		}
		dir = parent
	}
}

func TestDecode_ModesYML(t *testing.T) {
	root := repoRoot(t)
	path := filepath.Join(root, ".agent", "policies", "modes.yml")
	f, err := os.Open(path)
	if err != nil {
		t.Skipf("modes.yml not found: %v", err)
	}
	defer f.Close()

	got, err := Decode(f)
	if err != nil {
		t.Fatalf("Decode modes.yml: %v", err)
	}
	m, ok := got.(map[string]any)
	if !ok {
		t.Fatalf("expected map, got %T", got)
	}
	if m["id"] != "modes" {
		t.Errorf("id = %v, want modes", m["id"])
	}
	if _, ok := m["modes"]; !ok {
		t.Error("modes key missing from decoded output")
	}
	if _, ok := m["gate_catalog"]; !ok {
		t.Error("gate_catalog key missing from decoded output")
	}

	// Verify balanced mode has gates.
	modesMap, _ := m["modes"].(map[string]any)
	balanced, _ := modesMap["balanced"].(map[string]any)
	harness, _ := balanced["harness"].(map[string]any)
	if gates, ok := harness["gates"].([]any); !ok || len(gates) == 0 {
		t.Errorf("balanced.harness.gates = %v, want non-empty list", harness["gates"])
	}
}

func TestDecode_WorkflowBuildYML(t *testing.T) {
	root := repoRoot(t)
	path := filepath.Join(root, ".agent", "workflows", "build.yml")
	f, err := os.Open(path)
	if err != nil {
		t.Skipf("build.yml not found: %v", err)
	}
	defer f.Close()

	got, err := Decode(f)
	if err != nil {
		t.Fatalf("Decode build.yml: %v", err)
	}
	m := got.(map[string]any)
	if m["stage"] != "build" {
		t.Errorf("stage = %v, want build", m["stage"])
	}
	// Should have phases (a sequence of mappings).
	phases, ok := m["phases"].([]any)
	if !ok {
		t.Fatalf("phases = %T, want []any", m["phases"])
	}
	if len(phases) < 3 {
		t.Errorf("phases has %d items, want >= 3", len(phases))
	}
}

func TestDecode_PoliciesYML(t *testing.T) {
	root := repoRoot(t)
	path := filepath.Join(root, "harness", "policies.yml")
	f, err := os.Open(path)
	if err != nil {
		t.Skipf("policies.yml not found: %v", err)
	}
	defer f.Close()

	got, err := Decode(f)
	if err != nil {
		t.Fatalf("Decode policies.yml: %v", err)
	}
	m := got.(map[string]any)
	if m["max_file_lines"] != float64(500) {
		t.Errorf("max_file_lines = %v, want 500", m["max_file_lines"])
	}
	if m["enforce"] != "block" {
		t.Errorf("enforce = %v, want block", m["enforce"])
	}
}

// ── Round-trip consistency: Go YAML → Go JSON should match Python YAML → Python JSON ──

func TestToJSON_MatchesPythonShim(t *testing.T) {
	root := repoRoot(t)
	files := []string{
		".agent/policies/modes.yml",
		".agent/workflows/build.yml",
		".agent/workflows/design.yml",
		".agent/workflows/discover.yml",
		".agent/workflows/evolve.yml",
		".agent/workflows/review.yml",
		"harness/policies.yml",
	}
	for _, rel := range files {
		t.Run(rel, func(t *testing.T) {
			path := filepath.Join(root, rel)
			f, err := os.Open(path)
			if err != nil {
				t.Skipf("%s not found: %v", rel, err)
			}
			defer f.Close()

			// Decode with our Go YAML parser.
			goVal, err := Decode(f)
			if err != nil {
				t.Fatalf("Go YAML decode: %v", err)
			}
			f.Close()

			// Also decode with the Python shim for comparison.
			shimPath := filepath.Join(root, "harness", "yaml2json.py")
			if _, err := os.Stat(shimPath); err == nil {
				pyVal, err := decodeViaPython(shimPath, path)
				if err == nil {
					// Compare the two outputs by JSON bytes (canonical comparison).
					goJSON, _ := json.Marshal(goVal)
					pyJSON, _ := json.Marshal(pyVal)
					if !jsonBytesEqual(goJSON, pyJSON) {
						// Pretty-print both for debugging.
						goPretty, _ := json.MarshalIndent(goVal, "", "  ")
						pyPretty, _ := json.MarshalIndent(pyVal, "", "  ")
						t.Errorf("Go output differs from Python output for %s\nGo:  %s\nPy:  %s", rel, goPretty, pyPretty)
					}
				}
			}
		})
	}
}

// decodeViaPython runs the python yaml2json shim.
func decodeViaPython(shimPath, ymlPath string) (any, error) {
	cmd := exec.Command("python3", shimPath, ymlPath)
	out, err := cmd.Output()
	if err != nil {
		return nil, err
	}
	var val any
	if err := json.Unmarshal(out, &val); err != nil {
		return nil, err
	}
	return val, nil
}

// jsonBytesEqual compares two JSON byte sequences ignoring key ordering.
func jsonBytesEqual(a, b []byte) bool {
	var va, vb any
	if err := json.Unmarshal(a, &va); err != nil {
		return false
	}
	if err := json.Unmarshal(b, &vb); err != nil {
		return false
	}
	// Re-marshal for canonical comparison.
	ca, _ := json.Marshal(va)
	cb, _ := json.Marshal(vb)
	return bytes.Equal(ca, cb)
}
