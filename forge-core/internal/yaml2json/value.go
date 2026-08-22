package yaml2json

import (
	"fmt"
	"strings"
)

// ── Parser ────────────────────────────────────────────────────────────────

// parseDocument parses a YAML document starting at lines[pos]. Returns the
// decoded value and the index of the first unparsed line.
func parseDocument(lines []line, pos int) (any, int, error) {
	if pos >= len(lines) {
		return nil, pos, nil
	}
	return parseValue(lines, pos, -1)
}

// parseValue parses a YAML value starting at lines[pos], with a minimum
// indentation level (parentIndent). It handles mapping keys, sequence items,
// and scalar values by peeking at the current line.
func parseValue(lines []line, pos int, parentIndent int) (any, int, error) {
	if pos >= len(lines) {
		return nil, pos, nil
	}
	l := lines[pos]

	// If we've gone back to a lower indent than the parent, we're done.
	if parentIndent >= 0 && l.indent <= parentIndent {
		return nil, pos, nil
	}

	// Detect value type by examining the current line.
	text := strings.TrimSpace(l.text)

	// Sequence item: starts with "- " or "-" at end.
	if strings.HasPrefix(text, "- ") || text == "-" || strings.HasPrefix(text, "-\t") {
		return parseSequence(lines, pos, parentIndent)
	}

	// Mapping: contains ": " or ":" at end of key.
	if containsMapping(text) {
		return parseMapping(lines, pos, parentIndent)
	}

	if strings.HasPrefix(text, "[") || strings.HasPrefix(text, "{") {
		val, err := parseExactInlineValue(text)
		return val, pos + 1, err
	}

	// Scalar value.
	val := parseScalar(text)
	return val, pos + 1, nil
}

func parseExactInlineValue(text string) (any, error) {
	var val any
	sequence, rest, err := parseInlineSequence(text)
	val = sequence
	kind := "sequence"
	if strings.HasPrefix(text, "{") {
		mapping, mappingRest, mappingErr := parseInlineMapping(text)
		val, rest, err = mapping, mappingRest, mappingErr
		kind = "mapping"
	}
	if err != nil {
		return nil, err
	}
	if rest != "" {
		return nil, fmt.Errorf("trailing content after inline %s: %q", kind, rest)
	}
	return val, nil
}

// containsMapping checks if a line looks like a mapping key (contains " :" or ":" followed by space or end).
func containsMapping(text string) bool {
	// Must have ":" followed by space, tab, or end-of-line, and not be a
	// sequence item or inline structure.
	if strings.HasPrefix(text, "[") || strings.HasPrefix(text, "{") || strings.HasPrefix(text, "- ") || text == "-" {
		return false
	}
	// Try to find ": " or ":\t" or ":" at end.
	for i, c := range text {
		if c == ':' {
			// Check if it's a valid mapping separator.
			if i+1 < len(text) && (text[i+1] == ' ' || text[i+1] == '\t') {
				return true
			}
			if i+1 == len(text) {
				return true
			}
		}
	}
	return false
}

// parseMultilineValue parses a value that spans multiple lines at indent≥minIndent.
func parseMultilineValue(lines []line, pos int, minIndent int) (any, int, error) {
	if pos >= len(lines) {
		return nil, pos, nil
	}
	l := lines[pos]
	if l.indent < minIndent {
		return nil, pos, nil
	}
	return parseValue(lines, pos, minIndent-1)
}
