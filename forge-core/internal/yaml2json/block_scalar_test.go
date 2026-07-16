package yaml2json

import (
	"strings"
	"testing"
)

// ── Block scalar (| and >) regression tests ────────────────────────────────
//
// These guard the four bugs found by a fresh-context review of the
// normalize.go block-scalar refactor:
//   1. leaked "> "/"| " indicator prefix baked into the decoded value
//   2. folded (>) blocks losing relative/"more indented" line breaks
//   3. line.number not advancing past consumed block-scalar raw lines
//   4. (see yaml2json_test.go's TestToJSON_MatchesPythonShim — the
//      differential safety net itself, now a real t.Errorf on mismatch)

func TestDecode_BlockScalar_Literal(t *testing.T) {
	input := "key: |\n  line one\n  line two\n"
	val, err := Decode(strings.NewReader(input))
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	m, ok := val.(map[string]any)
	if !ok {
		t.Fatalf("expected map, got %T", val)
	}
	got, ok := m["key"].(string)
	if !ok {
		t.Fatalf("key value = %v (%T), want string", m["key"], m["key"])
	}
	if strings.HasPrefix(got, "|") {
		t.Errorf("decoded literal block leaked indicator prefix: %q", got)
	}
	want := "line one\nline two\n"
	if got != want {
		t.Errorf("literal block = %q, want %q", got, want)
	}
}

func TestDecode_BlockScalar_Folded(t *testing.T) {
	input := "description: >\n  hello world\n  more text\n"
	val, err := Decode(strings.NewReader(input))
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	m := val.(map[string]any)
	got, ok := m["description"].(string)
	if !ok {
		t.Fatalf("description = %v (%T), want string", m["description"], m["description"])
	}
	if strings.HasPrefix(got, ">") {
		t.Errorf("decoded folded block leaked indicator prefix: %q", got)
	}
	want := "hello world more text\n"
	if got != want {
		t.Errorf("folded block = %q, want %q", got, want)
	}
}

func TestDecode_BlockScalar_FoldedSingleLine(t *testing.T) {
	// The exact example from the bug report.
	input := "description: >\n  hello world\n"
	val, err := Decode(strings.NewReader(input))
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	m := val.(map[string]any)
	got := m["description"]
	want := "hello world\n"
	if got != want {
		t.Errorf("description = %q, want %q", got, want)
	}
}

func TestDecode_BlockScalar_FoldedMoreIndentedLine(t *testing.T) {
	// A folded block where one content line is indented deeper than its
	// siblings must keep a literal newline around that line (not fold it
	// to a space) — mirrors .agent/workflows/build.yml's mode_gating.note.
	input := "note: >\n  first line\n  second line\n   deeper line\n  back to normal\n"
	val, err := Decode(strings.NewReader(input))
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	m := val.(map[string]any)
	got, ok := m["note"].(string)
	if !ok {
		t.Fatalf("note = %v (%T), want string", m["note"], m["note"])
	}
	want := "first line second line\n deeper line\nback to normal\n"
	if got != want {
		t.Errorf("note = %q, want %q", got, want)
	}
}

func TestDecode_BlockScalar_LineNumberAdvancesPastBlock(t *testing.T) {
	// A block scalar spans multiple raw lines; the sibling key that follows
	// must report its true 1-based raw line number (used in error messages
	// like "line %d: invalid mapping").
	input := "note: >\n  one\n  two\n  three\nsibling: ok\n"
	lines := normalizeLines(input)
	if len(lines) != 2 {
		t.Fatalf("normalizeLines produced %d lines, want 2: %+v", len(lines), lines)
	}
	if lines[0].number != 1 {
		t.Errorf("block header line.number = %d, want 1", lines[0].number)
	}
	if lines[1].text != "sibling: ok" {
		t.Fatalf("lines[1].text = %q, want %q", lines[1].text, "sibling: ok")
	}
	if lines[1].number != 5 {
		t.Errorf("sibling line.number = %d, want 5 (raw line 5)", lines[1].number)
	}

	// End-to-end: decode should also succeed and produce both keys.
	val, err := Decode(strings.NewReader(input))
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	m := val.(map[string]any)
	if m["sibling"] != "ok" {
		t.Errorf("sibling = %v, want ok", m["sibling"])
	}
}

func TestDecode_BlockScalar_NoScalarCoercion(t *testing.T) {
	// A block scalar value that looks like a number, bool, or null must
	// stay a plain string — block scalars are always strings.
	tests := []struct {
		name  string
		input string
		key   string
		want  string
	}{
		{"looks-like-number", "value: |\n  123\n", "value", "123\n"},
		{"looks-like-null", "value: |\n  null\n", "value", "null\n"},
		{"looks-like-bool", "value: |\n  true\n", "value", "true\n"},
		{"looks-like-number-folded", "value: >\n  123\n", "value", "123\n"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			val, err := Decode(strings.NewReader(tt.input))
			if err != nil {
				t.Fatalf("Decode: %v", err)
			}
			m := val.(map[string]any)
			got, ok := m[tt.key].(string)
			if !ok {
				t.Fatalf("%s = %v (%T), want string (not coerced)", tt.key, m[tt.key], m[tt.key])
			}
			if got != tt.want {
				t.Errorf("%s = %q, want %q", tt.key, got, tt.want)
			}
		})
	}
}

func TestDecode_BlockScalar_NestedUnderKey(t *testing.T) {
	// A block scalar nested inside a mapping, with a sibling key after it —
	// matches the modes.yml/build.yml real-world shape.
	input := "outer:\n  note: >\n    line a\n    line b\n  after: done\n"
	val, err := Decode(strings.NewReader(input))
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	m := val.(map[string]any)
	outer, ok := m["outer"].(map[string]any)
	if !ok {
		t.Fatalf("outer = %T, want map", m["outer"])
	}
	if outer["note"] != "line a line b\n" {
		t.Errorf("outer.note = %q, want %q", outer["note"], "line a line b\n")
	}
	if outer["after"] != "done" {
		t.Errorf("outer.after = %v, want done", outer["after"])
	}
}

func TestDecode_BlockScalar_StripChomping(t *testing.T) {
	input := "key: |-\n  line one\n  line two\n"
	val, err := Decode(strings.NewReader(input))
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	m := val.(map[string]any)
	want := "line one\nline two"
	if m["key"] != want {
		t.Errorf("key = %q, want %q", m["key"], want)
	}
}

// TestDecode_BlockScalar_SequenceItemInlineMapping is a regression test: a
// block scalar used as the value of the FIRST key in a compact sequence-item
// mapping ("- key: |") must decode correctly, and a sibling key on the next
// line at the block's own indentation must survive as a separate map entry
// rather than being swallowed into the block's text. This previously failed
// because the block's baseIndent was computed from the "-" marker's column
// instead of the key's column, and separately because seqItemMapping never
// propagated the decoded block value through to the sequence item's map.
func TestDecode_BlockScalar_SequenceItemInlineMapping(t *testing.T) {
	input := "items:\n  - note: |\n      line one\n      line two\n    after: done\n"
	val, err := Decode(strings.NewReader(input))
	if err != nil {
		t.Fatalf("Decode: %v", err)
	}
	m := val.(map[string]any)
	items, ok := m["items"].([]any)
	if !ok || len(items) != 1 {
		t.Fatalf("items = %#v, want a 1-element slice", m["items"])
	}
	item, ok := items[0].(map[string]any)
	if !ok {
		t.Fatalf("items[0] = %T, want map", items[0])
	}
	if want := "line one\nline two\n"; item["note"] != want {
		t.Errorf("items[0].note = %q, want %q", item["note"], want)
	}
	if item["after"] != "done" {
		t.Errorf("items[0].after = %v, want \"done\" (sibling key must survive, not be swallowed into the block)", item["after"])
	}
}

