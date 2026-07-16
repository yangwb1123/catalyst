package yamlpath

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// ── Parse ─────────────────────────────────────────────────────────────────

func TestParse_Valid(t *testing.T) {
	tests := []struct {
		input    string
		wantFile string
		wantPath string
	}{
		{"../policies/modes.yml#workflow_depth.review", "../policies/modes.yml", "workflow_depth.review"},
		{"modes.yml#harness.gates", "modes.yml", "harness.gates"},
		{"/abs/path/modes.yml#a.b.c", "/abs/path/modes.yml", "a.b.c"},
		{"file.yml#single", "file.yml", "single"},
		{"deep/nested/path.yml#very.deeply.nested.field", "deep/nested/path.yml", "very.deeply.nested.field"},
	}
	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			got, err := Parse(tt.input)
			if err != nil {
				t.Fatalf("Parse(%q) = error %v", tt.input, err)
			}
			if got.File != tt.wantFile || got.Path != tt.wantPath {
				t.Errorf("Parse(%q) = {%q, %q}, want {%q, %q}",
					tt.input, got.File, got.Path, tt.wantFile, tt.wantPath)
			}
			if got.String() != tt.input {
				t.Errorf("Round-trip String() = %q, want %q", got.String(), tt.input)
			}
		})
	}
}

func TestParse_Invalid(t *testing.T) {
	tests := []struct {
		input string
		desc  string
	}{
		{"no-hash", "missing # separator"},
		{"#onlypath", "empty file path"},
		{"file.yml#", "empty field path"},
		{"", "empty string"},
	}
	for _, tt := range tests {
		t.Run(tt.desc, func(t *testing.T) {
			_, err := Parse(tt.input)
			if err == nil {
				t.Errorf("Parse(%q) = nil error, want error", tt.input)
			}
		})
	}
}

func TestMustParse_Valid(t *testing.T) {
	r := MustParse("../policies/modes.yml#workflow_depth.review")
	if r.File != "../policies/modes.yml" || r.Path != "workflow_depth.review" {
		t.Errorf("MustParse = {%q, %q}, want {../policies/modes.yml, workflow_depth.review}", r.File, r.Path)
	}
}

func TestMustParse_Panics(t *testing.T) {
	defer func() {
		if r := recover(); r == nil {
			t.Error("MustParse of invalid ref did not panic")
		}
	}()
	MustParse("bad-ref")
}

// ── WalkPath ──────────────────────────────────────────────────────────────

// TestWalkPath exercises the pure in-memory path walking (no file I/O).
func TestWalkPath_Map(t *testing.T) {
	doc := map[string]any{
		"a": map[string]any{
			"b": map[string]any{
				"c": "hello",
			},
		},
	}
	got, err := walkPath(doc, strings.Split("a.b.c", "."))
	if err != nil {
		t.Fatalf("walkPath: %v", err)
	}
	if s, ok := got.(string); !ok || s != "hello" {
		t.Errorf("walkPath result = %v (%T), want \"hello\" (string)", got, got)
	}
}

func TestWalkPath_Array(t *testing.T) {
	doc := map[string]any{
		"items": []any{"zero", "one", "two"},
	}
	got, err := walkPath(doc, strings.Split("items.2", "."))
	if err != nil {
		t.Fatalf("walkPath: %v", err)
	}
	if s, ok := got.(string); !ok || s != "two" {
		t.Errorf("walkPath result = %v (%T), want \"two\" (string)", got, got)
	}
}

func TestWalkPath_NestedArray(t *testing.T) {
	doc := map[string]any{
		"groups": []any{
			[]any{"a", "b"},
			[]any{"c", "d"},
		},
	}
	got, err := walkPath(doc, strings.Split("groups.1.0", "."))
	if err != nil {
		t.Fatalf("walkPath: %v", err)
	}
	if s, ok := got.(string); !ok || s != "c" {
		t.Errorf("walkPath result = %v (%T), want \"c\" (string)", got, got)
	}
}

func TestWalkPath_Scalar(t *testing.T) {
	doc := map[string]any{
		"count": float64(42),
	}
	got, err := walkPath(doc, strings.Split("count", "."))
	if err != nil {
		t.Fatalf("walkPath: %v", err)
	}
	if n, ok := got.(float64); !ok || n != 42 {
		t.Errorf("walkPath result = %v (%T), want 42 (float64)", got, got)
	}
}

func TestWalkPath_NilValue(t *testing.T) {
	doc := map[string]any{
		"maybe": nil,
	}
	_, err := walkPath(doc, strings.Split("maybe.nested", "."))
	if err == nil {
		t.Error("walkPath into nil value: got nil error, want error")
	}
}

func TestWalkPath_MissingKey(t *testing.T) {
	doc := map[string]any{"a": map[string]any{"b": 1}}
	_, err := walkPath(doc, strings.Split("a.c", "."))
	if err == nil {
		t.Error("walkPath to missing key: got nil error, want error")
	}
}

func TestWalkPath_OutOfBounds(t *testing.T) {
	doc := map[string]any{"items": []any{"a"}}
	_, err := walkPath(doc, strings.Split("items.5", "."))
	if err == nil {
		t.Error("walkPath out-of-bounds: got nil error, want error")
	}
}

func TestWalkPath_DescendIntoScalar(t *testing.T) {
	doc := map[string]any{"x": "hello"}
	_, err := walkPath(doc, strings.Split("x.y", "."))
	if err == nil {
		t.Error("walkPath descending into string: got nil error, want error")
	}
}

// ── Resolve (integration with real yaml2json shim) ─────────────────────────

// requireShim skips the test when the real yaml2json.py is not available.
func requireShim(t *testing.T, root string) {
	t.Helper()
	shim := ShimPath(root)
	if _, err := os.Stat(shim); err != nil {
		t.Skipf("yaml2json shim not found at %s: %v", shim, err)
	}
}

// repoRoot attempts to find the ForgeOS repo root, skipping when not inside it.
func repoRoot(t *testing.T) string {
	t.Helper()
	// Walk up from the test's working directory looking for harness/yaml2json.py.
	dir, err := os.Getwd()
	if err != nil {
		t.Skip("cannot get cwd")
	}
	for {
		if _, err := os.Stat(filepath.Join(dir, "harness", "yaml2json.py")); err == nil {
			return dir
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			t.Skip("not inside ForgeOS repo (harness/yaml2json.py not found in any parent)")
		}
		dir = parent
	}
}

func TestResolve_WorkflowDepthReview(t *testing.T) {
	root := repoRoot(t)
	requireShim(t, root)
	// Resolve "../policies/modes.yml#modes.balanced.workflow_depth.review" from the workflows dir.
	wfDir := filepath.Join(root, ".agent", "workflows")
	got, err := Resolve(root, "../policies/modes.yml#modes.balanced.workflow_depth.review", wfDir)
	if err != nil {
		t.Fatalf("Resolve: %v", err)
	}
	// Balanced mode's workflow_depth.review should be a string.
	switch v := got.(type) {
	case string:
		if v == "" {
			t.Errorf("Resolve review depth = empty string, want non-empty")
		}
	default:
		t.Errorf("Resolve review depth = %T(%v), want string", got, got)
	}
}

func TestResolve_HarnessGates(t *testing.T) {
	root := repoRoot(t)
	requireShim(t, root)
	wfDir := filepath.Join(root, ".agent", "workflows")
	got, err := Resolve(root, "../policies/modes.yml#modes.balanced.harness.gates", wfDir)
	if err != nil {
		t.Fatalf("Resolve: %v", err)
	}
	// Should be a list of gate names.
	switch v := got.(type) {
	case []any:
		if len(v) == 0 {
			t.Error("Resolve harness gates = empty list, want non-empty")
		}
	default:
		t.Errorf("Resolve harness gates = %T(%v), want []any", got, got)
	}
}

func TestResolve_InvalidRef(t *testing.T) {
	root := repoRoot(t)
	requireShim(t, root)
	wfDir := filepath.Join(root, ".agent", "workflows")
	_, err := Resolve(root, "badref", wfDir)
	if err == nil {
		t.Error("Resolve of invalid ref: got nil error, want error")
	}
}

func TestResolve_MissingFile(t *testing.T) {
	root := repoRoot(t)
	requireShim(t, root)
	wfDir := filepath.Join(root, ".agent", "workflows")
	_, err := Resolve(root, "nonexistent.yml#some.path", wfDir)
	if err == nil {
		t.Error("Resolve of missing file: got nil error, want error")
	}
}

func TestResolve_BadPath(t *testing.T) {
	root := repoRoot(t)
	requireShim(t, root)
	wfDir := filepath.Join(root, ".agent", "workflows")
	_, err := Resolve(root, "../policies/modes.yml#nonexistent.field.that.does.not.exist", wfDir)
	if err == nil {
		t.Error("Resolve of bad field path: got nil error, want error")
	}
}

// ── Edge cases ────────────────────────────────────────────────────────────

func TestParseIndex_Valid(t *testing.T) {
	tests := []struct {
		input string
		want  int
	}{
		{"0", 0},
		{"1", 1},
		{"42", 42},
		{"999", 999},
	}
	for _, tt := range tests {
		got, err := parseIndex(tt.input)
		if err != nil {
			t.Errorf("parseIndex(%q) = error %v, want %d", tt.input, err, tt.want)
		}
		if got != tt.want {
			t.Errorf("parseIndex(%q) = %d, want %d", tt.input, got, tt.want)
		}
	}
}

func TestParseIndex_Invalid(t *testing.T) {
	tests := []string{"-1", "abc", "12x", "", "9999999999999999999999999999"}
	for _, input := range tests {
		_, err := parseIndex(input)
		if err == nil {
			t.Errorf("parseIndex(%q) = nil error, want error", input)
		}
	}
}
