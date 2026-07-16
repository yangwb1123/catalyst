package yaml2json

import "strings"

// ── Lexer / line structure ────────────────────────────────────────────────

// line carries one logical YAML line with its indentation level.
type line struct {
	indent     int     // indentation level (spaces)
	text       string  // trimmed text (no leading whitespace)
	raw        string  // original line (for error messages)
	number     int     // 1-based line number
	blockValue *string // non-nil when text is "key:" for a block scalar
	// (| or >): the already-decoded literal VALUE. Mapping parsing must use
	// this verbatim and must NOT re-run it through scalar coercion
	// (parseInlineValue/parseScalar), since block scalars are always strings.
}

// normalizeLines splits YAML text into logical lines, stripping comments
// and normalizing indentation (tabs → 2 spaces). It also handles multi-line
// literal blocks (|) and folded blocks (>), consuming subsequent indented
// lines into a single line with \n-joined content.
func normalizeLines(data string) []line {
	rawLines := strings.Split(data, "\n")
	var lines []line
	outputLine := 0
	i := 0
	for i < len(rawLines) {
		raw := rawLines[i]
		outputLine++

		// Strip BOM from first line.
		if i == 0 {
			raw = strings.TrimLeft(raw, "\ufeff")
		}
		// Normalize leading tabs to 2 spaces.
		expanded := expandLeadingTabs(raw)
		// Calculate indent (spaces before first non-space character).
		trimmed := strings.TrimLeft(expanded, " ")
		indent := len(expanded) - len(trimmed)
		// Strip inline comment (but not # inside quotes).
		trimmed = stripComment(trimmed)
		trimmed = strings.TrimRight(trimmed, " \t\r")

		// Check for multi-line scalar indicator (| or >).
		if isBlockScalarIndicator(trimmed) {
			keyPart, indicator, chomp := parseBlockHeader(trimmed)
			blockText, newI := consumeBlockScalar(rawLines, i+1, indent, indicator, chomp)
			lines = append(lines, line{indent: indent, text: keyPart, raw: raw, number: outputLine, blockValue: &blockText})
			// A block scalar can consume many raw lines at once (newI jumps
			// past i+1); advance outputLine by however many extra raw lines
			// were swallowed so every subsequent line.number stays accurate.
			outputLine += newI - (i + 1)
			i = newI
		} else if trimmed == "" {
			// Skip empty lines and bare comments.
			i++
			continue
		} else {
			lines = append(lines, line{indent: indent, text: trimmed, raw: raw, number: outputLine})
			i++
		}
	}
	return lines
}

// expandLeadingTabs normalizes leading tab characters in raw to 2 spaces
// each, leaving the rest of the line untouched.
func expandLeadingTabs(raw string) string {
	expanded := raw
	tabCount := 0
	for strings.HasPrefix(expanded, "\t") {
		tabCount++
		expanded = expanded[1:]
	}
	if tabCount > 0 {
		expanded = strings.Repeat(" ", tabCount*2) + expanded
	}
	return expanded
}

// blockScalarSuffixes are the recognized block-scalar header suffixes,
// longest first so a chomping suffix (|- |+ >- >+) is matched before its
// bare one-character indicator (| >).
var blockScalarSuffixes = []string{"|-", "|+", ">-", ">+", "|", ">"}

// isBlockScalarIndicator reports whether trimmed ends with a YAML literal
// (|) or folded (>) block scalar indicator, with optional chomping suffix.
func isBlockScalarIndicator(trimmed string) bool {
	for _, suf := range blockScalarSuffixes {
		if strings.HasSuffix(trimmed, suf) {
			return true
		}
	}
	return false
}

// parseBlockHeader splits a block-scalar header line (e.g. "key: >" or
// "key: |-") into the key portion with the indicator removed ("key:"), the
// style indicator ('|' literal or '>' folded), and the chomping indicator
// (0 for default/clip, '-' for strip, '+' for keep). trimmed must satisfy
// isBlockScalarIndicator.
func parseBlockHeader(trimmed string) (keyPart string, indicator byte, chomp byte) {
	for _, suf := range blockScalarSuffixes {
		if strings.HasSuffix(trimmed, suf) {
			keyPart = strings.TrimRight(trimmed[:len(trimmed)-len(suf)], " \t")
			indicator = suf[0]
			if len(suf) == 2 {
				chomp = suf[1]
			}
			return keyPart, indicator, chomp
		}
	}
	return trimmed, '|', 0
}

// gatheredBlockLine is one raw line consumed into a block scalar, before
// folding/chomping is applied.
type gatheredBlockLine struct {
	indent int
	text   string // content with leading whitespace already stripped
	blank  bool
}

// consumeBlockScalar consumes rawLines[i:] while they are blank or indented
// deeper than baseIndent (the indentation of the "key: |"/"key: >" line that
// introduced the block), decodes them per YAML block-scalar folding and
// chomping rules, and returns the decoded VALUE (never including the "key:"
// or indicator) plus the index of the first unconsumed raw line.
func consumeBlockScalar(rawLines []string, i int, baseIndent int, indicator byte, chomp byte) (string, int) {
	var gathered []gatheredBlockLine
	for i < len(rawLines) {
		next := rawLines[i]
		expNext := expandLeadingTabs(next)
		nextTrimmed := strings.TrimLeft(expNext, " ")
		if nextTrimmed == "" {
			// Blank (or all-whitespace) line: tentatively part of the block;
			// trailing runs of these are trimmed later per chomp mode.
			gathered = append(gathered, gatheredBlockLine{blank: true})
			i++
			continue
		}
		nextIndent := len(expNext) - len(nextTrimmed)
		if nextIndent <= baseIndent {
			// Back to base indent: end of block scalar.
			break
		}
		gathered = append(gathered, gatheredBlockLine{indent: nextIndent, text: nextTrimmed})
		i++
	}
	return decodeBlockLines(gathered, indicator, chomp), i
}

// blockEntry is a gatheredBlockLine after content-indentation has been
// resolved: content is the line's text with the block's common indentation
// stripped (retaining any deeper, "more indented" whitespace literally).
type blockEntry struct {
	blank   bool
	more    bool // indented deeper than the block's content indentation
	content string
}

// decodeBlockLines applies YAML block-scalar indentation-stripping, folding
// (for '>'), and chomping ('-'/default-clip/'+') to gathered, producing the
// final decoded scalar string.
func decodeBlockLines(gathered []gatheredBlockLine, indicator byte, chomp byte) string {
	if len(gathered) == 0 {
		return ""
	}
	contentIndent := blockContentIndent(gathered)
	if contentIndent < 0 {
		// Nothing but blank lines: no real content.
		if chomp == '+' {
			return strings.Repeat("\n", len(gathered))
		}
		return ""
	}

	entries, lastContent := buildBlockEntries(gathered, contentIndent)
	fold := indicator == '>'
	switch chomp {
	case '-':
		return foldBlockEntries(entries, lastContent+1, fold)
	case '+':
		return foldBlockEntries(entries, len(entries), fold) + "\n"
	default: // clip
		return foldBlockEntries(entries, lastContent+1, fold) + "\n"
	}
}

// blockContentIndent returns the indentation of gathered's first non-blank
// line (the block's content indentation), or -1 if gathered has no content
// (every line is blank).
func blockContentIndent(gathered []gatheredBlockLine) int {
	for _, g := range gathered {
		if !g.blank {
			return g.indent
		}
	}
	return -1
}

// buildBlockEntries resolves each gathered line's content relative to
// contentIndent, retaining any deeper ("more indented") whitespace
// literally, and reports the index of the last non-blank entry.
func buildBlockEntries(gathered []gatheredBlockLine, contentIndent int) ([]blockEntry, int) {
	entries := make([]blockEntry, len(gathered))
	lastContent := -1
	for k, g := range gathered {
		if g.blank {
			entries[k] = blockEntry{blank: true}
			continue
		}
		pad := g.indent - contentIndent
		if pad < 0 {
			pad = 0
		}
		entries[k] = blockEntry{content: strings.Repeat(" ", pad) + g.text, more: g.indent > contentIndent}
		lastContent = k
	}
	return entries, lastContent
}

// foldBlockEntries joins entries[:limit] per YAML block-scalar folding
// rules: a literal newline between any pair where either side is blank or
// "more indented" than the block's content indentation; otherwise (fold
// mode only) a single folded space between two ordinary content lines.
func foldBlockEntries(entries []blockEntry, limit int, fold bool) string {
	var b strings.Builder
	for k := 0; k < limit; k++ {
		if k > 0 {
			prev, cur := entries[k-1], entries[k]
			if fold && !prev.more && !cur.more && !prev.blank && !cur.blank {
				b.WriteByte(' ')
			} else {
				b.WriteByte('\n')
			}
		}
		b.WriteString(entries[k].content)
	}
	return b.String()
}

// stripComment removes the YAML comment portion (# ...) from a line, being
// careful not to strip # characters inside quoted strings.
func stripComment(s string) string {
	inSingle := false
	inDouble := false
	for i, c := range s {
		if c == '\'' && !inDouble {
			inSingle = !inSingle
		} else if c == '"' && !inSingle {
			inDouble = !inDouble
		} else if c == '#' && !inSingle && !inDouble {
			// Check if this is a URL or inline comment.
			// YAML allows # only as comment start when preceded by whitespace
			// or at the start of a value.
			if i == 0 || s[i-1] == ' ' || s[i-1] == '\t' {
				return strings.TrimRight(s[:i], " \t")
			}
		}
	}
	return s
}
