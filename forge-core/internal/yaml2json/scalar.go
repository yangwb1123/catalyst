package yaml2json

import (
	"strconv"
	"strings"
)

// ── Scalar parsing ────────────────────────────────────────────────────────

// parseScalar converts a YAML scalar string to a Go value.
func parseScalar(text string) any {
	text = strings.TrimSpace(text)
	if text == "" {
		return nil
	}
	if text == "~" || text == "null" || text == "Null" || text == "NULL" {
		return nil
	}
	if text == "true" || text == "True" || text == "TRUE" || text == "yes" || text == "Yes" || text == "YES" || text == "on" || text == "On" || text == "ON" {
		return true
	}
	if text == "false" || text == "False" || text == "FALSE" || text == "no" || text == "No" || text == "NO" || text == "off" || text == "Off" || text == "OFF" {
		return false
	}
	if text == ".inf" || text == ".Inf" || text == ".INF" || text == "+.inf" || text == "+.Inf" || text == "+.INF" {
		return "inf"
	}
	if text == "-.inf" || text == "-.Inf" || text == "-.INF" {
		return "-inf"
	}
	if text == ".nan" || text == ".NaN" || text == ".NAN" {
		return "nan"
	}

	// Quoted string: strip quotes.
	if len(text) >= 2 {
		if (text[0] == '\'' && text[len(text)-1] == '\'') ||
			(text[0] == '"' && text[len(text)-1] == '"') {
			return text[1 : len(text)-1]
		}
	}

	// Number detection (int or float).
	if isNumeric(text) {
		if strings.Contains(text, ".") || strings.Contains(text, "e") || strings.Contains(text, "E") {
			if f, err := strconv.ParseFloat(text, 64); err == nil {
				return f
			}
		} else {
			if i, err := strconv.ParseInt(text, 10, 64); err == nil {
				return float64(i) // JSON numbers are float64
			}
		}
	}

	return text
}

// isNumeric checks if a string looks like a number.
func isNumeric(s string) bool {
	if s == "" {
		return false
	}
	// Allow leading sign.
	start := 0
	if s[0] == '+' || s[0] == '-' {
		start = 1
		if len(s) <= 1 {
			return false
		}
	}
	hasDigit := false
	hasDot := false
	hasExp := false
	for i := start; i < len(s); i++ {
		c := s[i]
		if c >= '0' && c <= '9' {
			hasDigit = true
		} else if c == '.' && !hasDot && !hasExp {
			hasDot = true
		} else if (c == 'e' || c == 'E') && !hasExp && hasDigit {
			hasExp = true
			hasDot = true // prevent second dot in exponent notation
			if i+1 < len(s) && (s[i+1] == '+' || s[i+1] == '-') {
				i++ // skip sign after exponent
			}
		} else {
			return false
		}
	}
	return hasDigit
}
