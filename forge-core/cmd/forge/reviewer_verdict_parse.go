package main

import (
	"encoding/json"
	"fmt"
	"io"
	"strings"
	"unicode"
	"unicode/utf8"
)

func parseStrictReviewerVerdict(output string, requireEnvelope bool) (string, bool) {
	if !validStrictReviewerText(output) {
		return "", false
	}
	payload := output
	if requireEnvelope {
		var err error
		payload, err = successfulClaudeResultPayload(output)
		if err != nil {
			return "", false
		}
	}
	if !validStrictReviewerText(payload) || hasConflictingReviewerVerdict(payload) {
		return "", false
	}
	switch lastNonEmptyExactLine(payload) {
	case "VERDICT: APPROVE":
		return VerdictApprove, true
	case "VERDICT: REQUEST_CHANGES":
		return VerdictRequestChanges, true
	default:
		return "", false
	}
}

func validStrictReviewerText(text string) bool {
	if !utf8.ValidString(text) {
		return false
	}
	for index, r := range text {
		if r == '\r' {
			if index+1 >= len(text) || text[index+1] != '\n' {
				return false
			}
			continue
		}
		if unicode.IsControl(r) && r != '\n' && r != '\t' {
			return false
		}
	}
	return true
}

func hasConflictingReviewerVerdict(text string) bool {
	count := 0
	for _, line := range strings.Split(text, "\n") {
		line = strings.TrimSuffix(line, "\r")
		if line == "VERDICT: APPROVE" || line == "VERDICT: REQUEST_CHANGES" {
			count++
		}
	}
	return count != 1
}

func rejectDuplicateEnvelopeKeys(output string) error {
	dec := json.NewDecoder(strings.NewReader(strings.TrimSpace(output)))
	token, err := dec.Token()
	if err != nil || token != json.Delim('{') {
		return fmt.Errorf("result envelope must be one JSON object")
	}
	seen := map[string]bool{}
	for dec.More() {
		keyToken, err := dec.Token()
		key, ok := keyToken.(string)
		if err != nil || !ok || seen[key] {
			return fmt.Errorf("result envelope has an invalid or duplicate key")
		}
		if canonical, collision := nonCanonicalEnvelopeKey(key); collision {
			return fmt.Errorf("result envelope key %q aliases canonical key %q", key, canonical)
		}
		seen[key] = true
		var value json.RawMessage
		if err := dec.Decode(&value); err != nil {
			return fmt.Errorf("result envelope value: %w", err)
		}
	}
	if token, err = dec.Token(); err != nil || token != json.Delim('}') {
		return fmt.Errorf("result envelope is incomplete")
	}
	var trailing json.RawMessage
	if err := dec.Decode(&trailing); err != io.EOF {
		return fmt.Errorf("result envelope has trailing JSON")
	}
	return nil
}

func nonCanonicalEnvelopeKey(key string) (string, bool) {
	for _, canonical := range []string{"type", "subtype", "is_error", "result"} {
		if key != canonical && strings.EqualFold(key, canonical) {
			return canonical, true
		}
	}
	return "", false
}
