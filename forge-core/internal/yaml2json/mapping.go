package yaml2json

import (
	"fmt"
	"strings"
)

// ── Mapping parsing ───────────────────────────────────────────────────────

// parseMapping parses a YAML mapping starting at lines[pos].
func parseMapping(lines []line, pos int, parentIndent int) (map[string]any, int, error) {
	m := make(map[string]any)
	mappingIndent := lines[pos].indent

	for pos < len(lines) {
		l := lines[pos]
		if parentIndent >= 0 && l.indent <= parentIndent {
			break
		}
		if l.indent < mappingIndent {
			break
		}
		if l.indent > mappingIndent {
			// Sub-key continuation — should be handled by parseMappingContinuation.
			break
		}
		text := strings.TrimLeft(l.text, " ")
		if !containsMapping(text) {
			break
		}

		// Extract key and value separator position.
		sepPos := findMappingSep(text)
		if sepPos < 0 {
			return nil, pos, fmt.Errorf("line %d: invalid mapping: %q", l.number, l.raw)
		}
		key := strings.TrimSpace(text[:sepPos])
		rest := strings.TrimSpace(text[sepPos+1:])
		pos++

		val, newPos, err := resolveMappingValue(lines, l, pos, rest, mappingIndent+1)
		if err != nil {
			return nil, newPos, err
		}
		pos = newPos
		m[key] = val
	}
	if len(m) == 0 {
		return nil, pos, nil
	}
	return m, pos, nil
}

// resolveMappingValue determines the value for one "key: rest" mapping
// entry at line l (pos already points past l). It is shared by parseMapping
// and parseMappingContinuation:
//   - a block scalar (| or >) value is already fully decoded on l — used
//     verbatim, never re-run through scalar coercion (block scalars are
//     always strings, even if they look like a number/bool/null);
//   - an empty rest means the value is on subsequent, more-indented lines;
//   - otherwise rest is an inline scalar/[seq]/{map}.
func resolveMappingValue(lines []line, l line, pos int, rest string, minIndent int) (any, int, error) {
	switch {
	case l.blockValue != nil:
		return *l.blockValue, pos, nil
	case rest == "":
		return parseMultilineValue(lines, pos, minIndent)
	default:
		return parseInlineValue(rest), pos, nil
	}
}

// findMappingSep finds the position of ":" that separates key from value.
func findMappingSep(text string) int {
	// Scan for ":" that's a mapping separator (not inside quotes).
	inSingle := false
	inDouble := false
	for i, c := range text {
		if c == '\'' && !inDouble {
			inSingle = !inSingle
		} else if c == '"' && !inSingle {
			inDouble = !inDouble
		} else if c == ':' && !inSingle && !inDouble {
			return i
		}
	}
	return -1
}

// parseSimpleMapping parses a single-line mapping like "key: value".
func parseSimpleMapping(text string) (map[string]any, error) {
	sepPos := findMappingSep(text)
	if sepPos < 0 {
		return nil, fmt.Errorf("not a mapping: %q", text)
	}
	key := strings.TrimSpace(text[:sepPos])
	rest := strings.TrimSpace(text[sepPos+1:])
	m := map[string]any{key: parseInlineValue(rest)}
	return m, nil
}

// parseMappingContinuation reads additional sub-keys at indent > parentIndent
// and adds them to the existing map m.
func parseMappingContinuation(lines []line, pos int, parentIndent int, m map[string]any) (map[string]any, int, error) {
	if m == nil {
		m = make(map[string]any)
	}
	for pos < len(lines) {
		l := lines[pos]
		if l.indent <= parentIndent {
			break
		}
		text := strings.TrimLeft(l.text, " ")
		if !containsMapping(text) {
			break
		}
		sepPos := findMappingSep(text)
		if sepPos < 0 {
			return m, pos, nil
		}
		key := strings.TrimSpace(text[:sepPos])
		rest := strings.TrimSpace(text[sepPos+1:])
		pos++

		val, newPos, err := resolveMappingValue(lines, l, pos, rest, l.indent+1)
		if err != nil {
			return m, newPos, err
		}
		pos = newPos
		m[key] = val
	}
	return m, pos, nil
}
