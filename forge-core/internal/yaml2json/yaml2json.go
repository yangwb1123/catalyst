// Package yaml2json converts a subset of YAML to JSON using only the Go
// standard library (eighth-wave-adr-decay.md §方向5: Go 原生 YAML 解析).
//
// This package handles the YAML subset used by ForgeOS configuration files:
//
//   - Maps (key: value) with indentation-based nesting
//   - Sequences with dash prefix (- item)
//   - Inline sequences: [a, b, c]
//   - Scalar values: strings (quoted and unquoted), numbers, booleans, null
//   - Multi-line literal (|) and folded (>) blocks
//   - Comments (#)
//   - Mixed indentation (tabs/spaces normalized)
//
// It deliberately does NOT support these YAML 1.x features:
//   - Anchors (&) and aliases (*) — not used by ForgeOS configs
//   - Merge keys (<<:) — not used by ForgeOS configs
//   - Tags (!!str, !binary) — not used
//   - Multi-document (---/...) — not used
//   - Complex keys (? ), sets, ordered maps — not used
//
// The output is a generic Go value (map[string]any, []any, string, float64,
// bool, nil) that can be serialized to JSON via encoding/json.
//
// The parser is split across several files by concern, all in this one
// package:
//
//   - yaml2json.go  — this file: public API (Decode/ToJSON).
//   - normalize.go  — lexer: raw text → normalized `line` slice.
//   - value.go      — top-level value dispatch (map vs sequence vs scalar).
//   - sequence.go   — "- item" sequence parsing.
//   - mapping.go    — "key: value" mapping parsing.
//   - inline.go     — inline [..]/{..} value parsing.
//   - scalar.go     — leaf scalar (string/number/bool/null) parsing.
package yaml2json

import (
	"encoding/json"
	"fmt"
	"io"
)

// Decode reads YAML data and returns the decoded value. The returned value
// is one of: map[string]any, []any, string, float64, bool, or nil.
func Decode(r io.Reader) (any, error) {
	data, err := io.ReadAll(r)
	if err != nil {
		return nil, fmt.Errorf("yaml2json: read: %w", err)
	}
	lines := normalizeLines(string(data))
	if len(lines) == 0 {
		return nil, nil
	}
	val, pos, err := parseDocument(lines, 0)
	if err != nil {
		return nil, fmt.Errorf("yaml2json: line 1: %w", err)
	}
	if pos != len(lines) {
		return nil, fmt.Errorf("yaml2json: line %d: unconsumed content %q", lines[pos].number, lines[pos].raw)
	}
	return val, nil
}

// ToJSON decodes YAML and returns the JSON encoding.
func ToJSON(r io.Reader) ([]byte, error) {
	val, err := Decode(r)
	if err != nil {
		return nil, err
	}
	if val == nil {
		return []byte("null\n"), nil
	}
	return json.MarshalIndent(val, "", "  ")
}
