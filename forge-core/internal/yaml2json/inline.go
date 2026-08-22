package yaml2json

import (
	"fmt"
	"strings"
)

// ── Inline value parsing ──────────────────────────────────────────────────

// parseInlineValue parses a value that appears inline after "key: value".
func parseInlineValue(text string) (any, error) {
	if text == "" {
		return nil, nil
	}
	// Inline sequence
	if strings.HasPrefix(text, "[") {
		val, rest, err := parseInlineSequence(text)
		if err == nil {
			if rest != "" {
				return nil, fmt.Errorf("trailing content after inline sequence: %q", rest)
			}
			return val, nil
		}
		return nil, err
	}
	// Inline mapping
	if strings.HasPrefix(text, "{") {
		val, rest, err := parseInlineMapping(text)
		if err == nil {
			if rest != "" {
				return nil, fmt.Errorf("trailing content after inline mapping: %q", rest)
			}
			return val, nil
		}
		return nil, err
	}
	// Scalar
	return parseScalar(text), nil
}

// parseInlineSequence parses [a, b, c] style sequences.
func parseInlineSequence(text string) ([]any, string, error) {
	if !strings.HasPrefix(text, "[") {
		return nil, text, fmt.Errorf("not an inline sequence: %q", text)
	}
	// Find matching close bracket.
	depth := 0
	end := -1
	for i, c := range text {
		if c == '[' {
			depth++
		} else if c == ']' {
			depth--
			if depth == 0 {
				end = i
				break
			}
		}
	}
	if end < 0 {
		return nil, text, fmt.Errorf("unclosed inline sequence: %q", text)
	}
	inner := text[1:end]
	rest := strings.TrimSpace(text[end+1:])
	items := splitInlineValues(inner)
	var seq []any
	for _, item := range items {
		item = strings.TrimSpace(item)
		if item == "" {
			continue
		}
		seq = append(seq, parseScalar(item))
	}
	if seq == nil {
		seq = []any{}
	}
	return seq, rest, nil
}

// parseInlineMapping parses {key: val, ...} style mappings.
func parseInlineMapping(text string) (map[string]any, string, error) {
	if !strings.HasPrefix(text, "{") {
		return nil, text, fmt.Errorf("not an inline mapping: %q", text)
	}
	depth := 0
	end := -1
	for i, c := range text {
		if c == '{' {
			depth++
		} else if c == '}' {
			depth--
			if depth == 0 {
				end = i
				break
			}
		}
	}
	if end < 0 {
		return nil, text, fmt.Errorf("unclosed inline mapping: %q", text)
	}
	inner := text[1:end]
	rest := strings.TrimSpace(text[end+1:])
	pairs := splitInlineValues(inner)
	m := make(map[string]any)
	for _, pair := range pairs {
		pair = strings.TrimSpace(pair)
		if pair == "" {
			continue
		}
		sepPos := findMappingSep(pair)
		if sepPos < 0 {
			continue
		}
		key := strings.TrimSpace(pair[:sepPos])
		val := strings.TrimSpace(pair[sepPos+1:])
		if _, exists := m[key]; exists {
			return nil, text, fmt.Errorf("duplicate inline mapping key %q", key)
		}
		m[key] = parseScalar(val)
	}
	return m, rest, nil
}

// parseInlineItem handles a sequence item with inline content.
func parseInlineItem(text string) (any, string, error) {
	if strings.HasPrefix(text, "[") {
		return parseInlineSequence(text)
	}
	if strings.HasPrefix(text, "{") {
		m, rest, err := parseInlineMapping(text)
		return m, rest, err
	}
	if containsMapping(text) {
		m, err := parseSimpleMapping(text)
		return m, text, err
	}
	return parseScalar(text), text, nil
}

// splitInlineValues splits comma-separated values, respecting quoted strings
// and nested brackets/braces.
func splitInlineValues(text string) []string {
	var items []string
	depth := 0
	start := 0
	inSingle := false
	inDouble := false
	for i, c := range text {
		if c == '\'' && !inDouble {
			inSingle = !inSingle
		} else if c == '"' && !inSingle {
			inDouble = !inDouble
		} else if !inSingle && !inDouble {
			switch c {
			case '[', '{':
				depth++
			case ']', '}':
				depth--
			case ',':
				if depth == 0 {
					items = append(items, text[start:i])
					start = i + 1
				}
			}
		}
	}
	items = append(items, text[start:])
	return items
}
