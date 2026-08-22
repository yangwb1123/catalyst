package legacygovernanceimportcontract

import (
	"bytes"
	"fmt"
	"regexp"
	"strconv"
	"strings"
)

var (
	memoryFields = map[string]bool{
		"_format": true, "kind": true, "topic": true, "detail": true,
		"iteration": true, "source": true, "confidence": true,
		"supersedes": true, "created_at_unix": true,
	}
	memoryRequired = []string{"kind", "topic", "detail", "iteration", "created_at_unix"}
	integerPattern = regexp.MustCompile(`^-?(0|[1-9][0-9]*)$`)
	decimalPattern = regexp.MustCompile(
		`^(-?)(0|[1-9][0-9]*)(?:\.([0-9]+))?(?:[eE]([+-]?[0-9]+))?$`,
	)
)

func parseMemoryJSONL(raw []byte) ([]map[string]any, [][]byte, error) {
	lines := bytes.Split(raw[:len(raw)-1], []byte{'\n'})
	if len(lines) > maxMemoryEntries {
		return nil, nil, fmt.Errorf("memory entry count exceeds %d", maxMemoryEntries)
	}
	entries := make([]map[string]any, 0, len(lines))
	for index, line := range lines {
		if len(line) == 0 || len(line) > maxMemoryLineBytes {
			return nil, nil, fmt.Errorf("memory line %d is blank or exceeds bound", index+1)
		}
		entry, err := parseMemoryLine(line, index+1)
		if err != nil {
			return nil, nil, err
		}
		entries = append(entries, entry)
	}
	return entries, lines, nil
}

func parseMemoryLine(raw []byte, ordinal int) (map[string]any, error) {
	value, err := parseStrictJSON(raw, maxMemoryLineBytes, true)
	if err != nil {
		return nil, fmt.Errorf("memory line %d: %w", ordinal, err)
	}
	entry, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("memory line %d must be one JSON object", ordinal)
	}
	for field := range entry {
		if !memoryFields[field] {
			return nil, fmt.Errorf("memory line %d has unknown field %q", ordinal, field)
		}
	}
	for _, field := range memoryRequired {
		if _, exists := entry[field]; !exists {
			return nil, fmt.Errorf("memory line %d lacks field %q", ordinal, field)
		}
	}
	if format, present := entry["_format"]; present && format != "forgeos.memory.v1" {
		return nil, fmt.Errorf("memory _format must be omitted or forgeos.memory.v1")
	}
	kind, ok := entry["kind"].(string)
	if !ok || (kind != "decision" && kind != "gap" && kind != "lesson") {
		return nil, fmt.Errorf("memory kind is outside frozen vocabulary")
	}
	return projectMemoryFields(entry, kind)
}

func projectMemoryFields(entry map[string]any, kind string) (map[string]any, error) {
	topic, err := requiredMemoryText(entry, "topic", maxSourceRefBytes)
	if err != nil {
		return nil, err
	}
	detail, err := requiredMemoryText(entry, "detail", maxMemoryLineBytes)
	if err != nil {
		return nil, err
	}
	iteration, err := memoryInteger(entry["iteration"], "iteration")
	if err != nil {
		return nil, err
	}
	created, err := memoryInteger(entry["created_at_unix"], "created_at_unix")
	if err != nil {
		return nil, err
	}
	confidence, err := memoryConfidence(entry)
	if err != nil {
		return nil, err
	}
	source, err := optionalMemoryText(entry, "source")
	if err != nil {
		return nil, err
	}
	supersedes, err := optionalMemoryText(entry, "supersedes")
	if err != nil {
		return nil, err
	}
	return map[string]any{
		"confidence": confidence, "created_at_unix": created, "declared_kind": kind,
		"declared_source": source, "declared_supersedes": supersedes,
		"declared_topic": topic, "detail": detail, "iteration": iteration,
		"legacy_format": entry["_format"],
	}, nil
}

func requiredMemoryText(entry map[string]any, field string, maximum int) (string, error) {
	value, ok := entry[field].(string)
	if !ok || validateWireString(value, maximum, true) != nil {
		return "", fmt.Errorf("memory %s must be bounded nonempty text", field)
	}
	return value, nil
}

func optionalMemoryText(entry map[string]any, field string) (any, error) {
	value, exists := entry[field]
	if !exists {
		return nil, nil
	}
	text, ok := value.(string)
	if !ok || validateWireString(text, maxSourceRefBytes, true) != nil {
		return nil, fmt.Errorf("memory %s must be bounded nonempty text", field)
	}
	return text, nil
}

func memoryInteger(value any, label string) (int64, error) {
	lexeme, ok := value.(numberLexeme)
	if !ok || !integerPattern.MatchString(string(lexeme)) {
		return 0, fmt.Errorf("memory %s must be a JSON integer", label)
	}
	integer, err := strconv.ParseInt(string(lexeme), 10, 64)
	if err != nil {
		return 0, fmt.Errorf("memory %s is outside signed int64", label)
	}
	return integer, nil
}

func memoryConfidence(entry map[string]any) (map[string]any, error) {
	value, exists := entry["confidence"]
	if !exists {
		return map[string]any{"presence": "omitted", "raw_number_lexeme": nil}, nil
	}
	lexeme, ok := value.(numberLexeme)
	if !ok || len(lexeme) == 0 || len(lexeme) > maxConfidenceLexeme ||
		!confidenceInRange(string(lexeme)) {
		return nil, fmt.Errorf("memory confidence must be a decimal number in range 0..1")
	}
	return map[string]any{"presence": "explicit", "raw_number_lexeme": string(lexeme)}, nil
}

func confidenceInRange(raw string) bool {
	parts := decimalPattern.FindStringSubmatch(raw)
	if parts == nil {
		return false
	}
	digits := strings.TrimLeft(parts[2]+parts[3], "0")
	if digits == "" {
		return true
	}
	if parts[1] == "-" {
		return false
	}
	exponent, overflow := parseExponent(parts[4])
	if overflow > 0 {
		return false
	}
	if overflow < 0 {
		return true
	}
	threshold := int64(1 - (len(digits) - len(parts[3])))
	if exponent < threshold {
		return true
	}
	if exponent > threshold || digits[0] != '1' {
		return false
	}
	return strings.Trim(digits[1:], "0") == ""
}

func parseExponent(raw string) (int64, int) {
	if raw == "" {
		return 0, 0
	}
	value, err := strconv.ParseInt(raw, 10, 64)
	if err == nil {
		return value, 0
	}
	if raw[0] == '-' {
		return 0, -1
	}
	return 0, 1
}
