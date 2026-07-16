package yaml2json

import "strings"

// ── Sequence parsing ──────────────────────────────────────────────────────

// parseSequence parses YAML sequence items starting at lines[pos].
// Each item starts with "- " at the same indentation level.
func parseSequence(lines []line, pos int, parentIndent int) ([]any, int, error) {
	var seq []any
	seqIndent := -1

	for pos < len(lines) {
		l := lines[pos]
		if parentIndent >= 0 && l.indent <= parentIndent {
			break
		}
		text := strings.TrimLeft(l.text, " ")
		if !isSeqItemLine(text) {
			// Not a sequence item: end of this sequence.
			break
		}

		if seqIndent < 0 {
			seqIndent = l.indent
		}

		// Strip the "- " prefix.
		itemText := strings.TrimSpace(text[1:]) // after "-"
		itemText = strings.TrimLeft(itemText, " \t")

		newPos, err := parseSeqItem(lines, itemText, pos, seqIndent, &seq)
		if err != nil {
			return nil, newPos, err
		}
		pos = newPos
	}
	if len(seq) == 0 {
		return nil, pos, nil
	}
	return seq, pos, nil
}

// isSeqItemLine reports whether text is a "- item" sequence line.
func isSeqItemLine(text string) bool {
	return strings.HasPrefix(text, "- ") || text == "-" || strings.HasPrefix(text, "-\t")
}

// parseSeqItem dispatches one sequence item (itemText, the text after the
// "- " marker) to the appropriate branch handler and appends its decoded
// value onto *seq. It returns the line position following the item.
//
// Every branch always appends, including the empty-item branch: a bare "-"
// with no nested mapping/sequence decodes to a Go nil, the same
// representation a literal scalar "null" produces (see parseScalar), so
// "a:\n  -\n  - null\n" and "a: [null, null]" both yield a 2-element
// sequence of [nil, nil].
func parseSeqItem(lines []line, itemText string, pos int, seqIndent int, seq *[]any) (int, error) {
	switch {
	case itemText == "":
		// Empty sequence item — could be a sub-mapping, sub-sequence, or a
		// bare null (when there's no further-indented content).
		pos++
		item, newPos, err := seqItemEmpty(lines, pos, seqIndent)
		if err != nil {
			return pos, err
		}
		*seq = append(*seq, item)
		return newPos, nil
	case strings.HasPrefix(itemText, "["):
		// Inline sequence as item.
		val, err := seqItemInlineSequence(itemText)
		if err != nil {
			return pos, err
		}
		*seq = append(*seq, val)
		return pos + 1, nil
	case containsMapping(itemText):
		// The item value is an inline mapping like "- key: val"
		val, newPos, err := seqItemMapping(lines, itemText, pos, seqIndent)
		if err != nil {
			return newPos, err
		}
		*seq = append(*seq, val)
		return newPos, nil
	default:
		// Simple scalar item, possibly followed by sub-keys.
		val, newPos, err := seqItemScalar(lines, itemText, pos, seqIndent)
		if err != nil {
			return newPos, err
		}
		*seq = append(*seq, val)
		return newPos, nil
	}
}

// seqItemEmpty handles a bare "-" sequence item, whose value comes entirely
// from subsequent more-indented lines (a nested mapping or sequence).
// pos must already point past the "-" line.
func seqItemEmpty(lines []line, pos int, seqIndent int) (any, int, error) {
	item, newPos, err := parseMultilineValue(lines, pos, seqIndent+1)
	if err != nil {
		return nil, pos, err
	}
	return item, newPos, nil
}

// seqItemInlineSequence handles a sequence item whose text is itself an
// inline sequence, e.g. "- [a, b, c]".
func seqItemInlineSequence(itemText string) (any, error) {
	val, rest, err := parseInlineSequence(itemText)
	if err != nil {
		return nil, err
	}
	if rest != "" {
		// There's content after the sequence — treat as mapping.
		item, _, err := parseInlineItem(itemText)
		if err == nil && item != nil {
			return item, nil
		}
	}
	return val, nil
}

// seqItemMapping handles a sequence item whose text starts a mapping, e.g.
// "- key: val", possibly followed by further indented sub-keys.
// pos must point at the "-" line itself (not yet advanced).
func seqItemMapping(lines []line, itemText string, pos int, seqIndent int) (any, int, error) {
	m, err := parseSimpleMapping(itemText)
	if err == nil && m != nil {
		pos++
		more, newPos, err := parseMappingContinuation(lines, pos, seqIndent+1, m)
		if err != nil {
			return nil, pos, err
		}
		return more, newPos, nil
	}
	return parseScalar(itemText), pos + 1, nil
}

// seqItemScalar handles a plain scalar sequence item, which may be followed
// by further-indented sub-keys turning it into a single-entry mapping.
// pos must point at the "-" line itself (not yet advanced).
func seqItemScalar(lines []line, itemText string, pos int, seqIndent int) (any, int, error) {
	val := parseScalar(itemText)
	pos++
	if pos < len(lines) && lines[pos].indent > seqIndent {
		// Item with sub-structure: wrap in a single-entry map.
		sub, newPos, err := parseMultilineValue(lines, pos, seqIndent+1)
		if err != nil {
			return nil, pos, err
		}
		if valStr, ok := val.(string); ok {
			// The scalar value + sub = a mapping with the scalar as key.
			m := map[string]any{valStr: sub}
			// Also check for more sub-keys at the same level.
			m, newPos2, err := parseMappingContinuation(lines, newPos, seqIndent+1, m)
			if err != nil {
				return nil, pos, err
			}
			return m, newPos2, nil
		}
		return val, newPos, nil
	}
	return val, pos, nil
}
