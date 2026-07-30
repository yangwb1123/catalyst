package migrationtxn

import (
	"bytes"
	"fmt"
	"strconv"
	"strings"

	"forgeos/forge-core/internal/migrate"
)

const (
	lifecycleProduction   = "production"
	promotionOperationID  = "lifecycle-production-v1"
	manualModeOperationID = "mode-engineering-v1"
	promotionBlockStart   = "<!-- forge:migration:explorer-to-engineering-v1:start -->"
	promotionBlockEnd     = "<!-- forge:migration:explorer-to-engineering-v1:end -->"
	lifecycleReceiptFile  = "lifecycle-production.v1.json"
	manualModeReceiptFile = "mode-engineering.v1.json"
)

var knownProjectModes = map[string]bool{
	"explorer": true, "balanced": true, "engineering": true, "cto": true,
}

var knownProjectLifecycles = map[string]bool{
	"idea": true, "mvp": true, "growth": true, lifecycleProduction: true,
}

type scalarLocation struct {
	start int
	end   int
	quote byte
}

type projectSelectors struct {
	mode           string
	lifecycle      string
	modeToken      scalarLocation
	lifecycleToken scalarLocation
}

func strictProjectSelectors(data []byte) (projectSelectors, error) {
	modeValue, modeToken, err := scanTopLevelScalar(data, "mode")
	if err != nil {
		return projectSelectors{}, err
	}
	lifecycle, lifecycleToken, err := scanTopLevelScalar(data, "lifecycle")
	if err != nil {
		return projectSelectors{}, err
	}
	if !knownProjectModes[modeValue] {
		return projectSelectors{}, fmt.Errorf("unknown persistent mode %q", modeValue)
	}
	if !knownProjectLifecycles[lifecycle] {
		return projectSelectors{}, fmt.Errorf("unknown persistent lifecycle %q", lifecycle)
	}
	return projectSelectors{
		mode: modeValue, lifecycle: lifecycle,
		modeToken: modeToken, lifecycleToken: lifecycleToken,
	}, nil
}

func scanTopLevelScalar(data []byte, key string) (string, scalarLocation, error) {
	var found bool
	var value string
	var location scalarLocation
	for offset := 0; offset <= len(data); {
		lineEnd, next := nextProjectLine(data, offset)
		line := data[offset:lineEnd]
		if len(line) > 0 && line[len(line)-1] == '\r' {
			line = line[:len(line)-1]
		}
		prefix := []byte(key + ":")
		if bytes.HasPrefix(line, prefix) {
			if found {
				return "", scalarLocation{}, fmt.Errorf("duplicate top-level %s key", key)
			}
			parsed, token, err := parseScalarField(line[len(prefix):])
			if err != nil {
				return "", scalarLocation{}, fmt.Errorf("invalid top-level %s: %w", key, err)
			}
			found, value = true, parsed
			location = scalarLocation{
				start: offset + len(prefix) + token.start,
				end:   offset + len(prefix) + token.end,
				quote: token.quote,
			}
		}
		if next < 0 {
			break
		}
		offset = next
	}
	if !found {
		return "", scalarLocation{}, fmt.Errorf("missing top-level %s key", key)
	}
	return value, location, nil
}

func nextProjectLine(data []byte, offset int) (lineEnd, next int) {
	if offset == len(data) {
		return offset, -1
	}
	relative := bytes.IndexByte(data[offset:], '\n')
	if relative < 0 {
		return len(data), -1
	}
	return offset + relative, offset + relative + 1
}

func parseScalarField(field []byte) (string, scalarLocation, error) {
	valueEnd, err := scalarCommentBoundary(field)
	if err != nil {
		return "", scalarLocation{}, err
	}
	start := 0
	for start < valueEnd && isYAMLSpace(field[start]) {
		start++
	}
	end := valueEnd
	for end > start && isYAMLSpace(field[end-1]) {
		end--
	}
	if start == end {
		return "", scalarLocation{}, fmt.Errorf("empty scalar")
	}
	value, quote, err := decodeScalarToken(field[start:end])
	if err != nil {
		return "", scalarLocation{}, err
	}
	return value, scalarLocation{start: start, end: end, quote: quote}, nil
}

func scalarCommentBoundary(field []byte) (int, error) {
	var quote byte
	escaped := false
	for i, b := range field {
		if quote == '"' && escaped {
			escaped = false
			continue
		}
		if quote == '"' && b == '\\' {
			escaped = true
			continue
		}
		if quote != 0 {
			if b == quote {
				quote = 0
			}
			continue
		}
		if b == '"' || b == '\'' {
			quote = b
			continue
		}
		if b == '#' {
			return i, nil
		}
	}
	if quote != 0 {
		return 0, fmt.Errorf("unterminated quoted scalar")
	}
	return len(field), nil
}

func decodeScalarToken(token []byte) (string, byte, error) {
	if len(token) >= 2 && token[0] == '"' && token[len(token)-1] == '"' {
		value, err := strconv.Unquote(string(token))
		if err != nil {
			return "", 0, fmt.Errorf("invalid double-quoted scalar: %w", err)
		}
		return value, '"', nil
	}
	if len(token) >= 2 && token[0] == '\'' && token[len(token)-1] == '\'' {
		inner := strings.ReplaceAll(string(token[1:len(token)-1]), "''", "'")
		return inner, '\'', nil
	}
	if bytes.IndexFunc(token, func(r rune) bool { return r == ' ' || r == '\t' }) >= 0 {
		return "", 0, fmt.Errorf("unquoted scalar contains whitespace")
	}
	if token[0] == '"' || token[0] == '\'' || token[len(token)-1] == '"' || token[len(token)-1] == '\'' {
		return "", 0, fmt.Errorf("mismatched scalar quotes")
	}
	return string(token), 0, nil
}

func isYAMLSpace(b byte) bool { return b == ' ' || b == '\t' }

func rewriteProjectSelectors(data []byte, selectors projectSelectors, mode, lifecycle string) []byte {
	type replacement struct {
		location scalarLocation
		value    string
	}
	replacements := []replacement{
		{location: selectors.modeToken, value: mode},
		{location: selectors.lifecycleToken, value: lifecycle},
	}
	if replacements[0].location.start < replacements[1].location.start {
		replacements[0], replacements[1] = replacements[1], replacements[0]
	}
	out := append([]byte(nil), data...)
	for _, replacement := range replacements {
		token := quoteScalar(replacement.value, replacement.location.quote)
		out = replaceBytes(out, replacement.location.start, replacement.location.end, token)
	}
	return out
}

func quoteScalar(value string, quote byte) []byte {
	switch quote {
	case '"':
		return []byte(strconv.Quote(value))
	case '\'':
		return []byte("'" + strings.ReplaceAll(value, "'", "''") + "'")
	default:
		return []byte(value)
	}
}

func replaceBytes(data []byte, start, end int, replacement []byte) []byte {
	out := make([]byte, 0, len(data)-(end-start)+len(replacement))
	out = append(out, data[:start]...)
	out = append(out, replacement...)
	out = append(out, data[end:]...)
	return out
}

func promotionRoadmapBlock(tasks []migrate.Task, newline string) []byte {
	var builder strings.Builder
	builder.WriteString(promotionBlockStart)
	builder.WriteString(newline)
	builder.WriteString("## Migration backfill (explorer -> engineering)")
	builder.WriteString(newline)
	builder.WriteString("> Derived by a durable Forge governance migration.")
	builder.WriteString(newline)
	for _, task := range tasks {
		builder.WriteString(promotionRoadmapLine(task))
		builder.WriteString(newline)
	}
	builder.WriteString(promotionBlockEnd)
	builder.WriteString(newline)
	return []byte(builder.String())
}

func promotionRoadmapLine(task migrate.Task) string {
	meta := task.Priority
	if task.Gate != "" {
		meta = "gate: " + task.Gate + ", " + task.Priority
	}
	return fmt.Sprintf("- [ ] [migrate] %s (%s) <!-- forge:migration-task:%s -->",
		task.Title, meta, task.ID)
}

func appendPromotionRoadmap(data []byte, tasks []migrate.Task) ([]byte, error) {
	present, err := validatePromotionRoadmap(data, tasks)
	if err != nil {
		return nil, err
	}
	if present {
		return nil, fmt.Errorf("promotion task block already exists without a matching terminal receipt")
	}
	newline := roadmapNewline(data)
	out := append([]byte(nil), data...)
	if len(out) > 0 && !bytes.HasSuffix(out, []byte(newline)) {
		out = append(out, []byte(newline)...)
	}
	if len(out) > 0 {
		out = append(out, []byte(newline)...)
	}
	return append(out, promotionRoadmapBlock(tasks, newline)...), nil
}

func validatePromotionRoadmap(data []byte, tasks []migrate.Task) (bool, error) {
	startCount := bytes.Count(data, []byte(promotionBlockStart))
	endCount := bytes.Count(data, []byte(promotionBlockEnd))
	markerCount := startCount + endCount
	for _, task := range tasks {
		markerCount += bytes.Count(data, []byte("forge:migration-task:"+task.ID))
	}
	if markerCount == 0 {
		return false, nil
	}
	if startCount != 1 || endCount != 1 {
		return false, fmt.Errorf("promotion task block markers are incomplete or duplicated")
	}
	start := bytes.Index(data, []byte(promotionBlockStart))
	end := bytes.Index(data, []byte(promotionBlockEnd))
	if start >= end {
		return false, fmt.Errorf("promotion task block markers are out of order")
	}
	for _, task := range tasks {
		marker := []byte("forge:migration-task:" + task.ID)
		if bytes.Count(data, marker) != 1 {
			return false, fmt.Errorf("promotion task marker %q is missing or duplicated", task.ID)
		}
	}
	block := promotionBlockSlice(data, start, end)
	normalized := strings.ReplaceAll(string(block), "\r\n", "\n")
	normalized = normalizePromotionCheckboxes(normalized)
	expected := string(promotionRoadmapBlock(tasks, "\n"))
	if normalized != expected {
		return false, fmt.Errorf("promotion task block content or order drifted")
	}
	return true, nil
}

func promotionBlockSlice(data []byte, start, end int) []byte {
	blockEnd := end + len(promotionBlockEnd)
	switch {
	case bytes.HasPrefix(data[blockEnd:], []byte("\r\n")):
		blockEnd += 2
	case bytes.HasPrefix(data[blockEnd:], []byte("\n")):
		blockEnd++
	}
	return data[start:blockEnd]
}

func normalizePromotionCheckboxes(block string) string {
	lines := strings.Split(block, "\n")
	for index, line := range lines {
		if strings.HasPrefix(line, "- [x] [migrate]") ||
			strings.HasPrefix(line, "- [X] [migrate]") {
			lines[index] = "- [ ]" + line[len("- [x]"):]
		}
	}
	return strings.Join(lines, "\n")
}

func roadmapNewline(data []byte) string {
	if index := bytes.IndexByte(data, '\n'); index > 0 && data[index-1] == '\r' {
		return "\r\n"
	}
	return "\n"
}
