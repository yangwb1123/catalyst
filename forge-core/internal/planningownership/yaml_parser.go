package planningownership

import (
	"fmt"
	"regexp"
	"strconv"
	"strings"
)

var numericLikePattern = regexp.MustCompile(`^[+-]?(?:[0-9].*|\.[0-9].*)$`)
var timestampPattern = regexp.MustCompile(`^(?:[0-9]{4}-[0-9]{1,2}-[0-9]{1,2}(?:[Tt ].*)?|[0-9]{1,2}:[0-9]{2}(?::[0-9]{2})?(?:\.[0-9]+)?(?:[Zz]|[+-][0-9:]+)?)$`)

type yamlLine struct {
	number, indent int
	text           string
	blank          bool
}

type yamlParser struct {
	lines                         []yamlLine
	position, tokens, collections int
}

func decodeStrictYAML(raw []byte, maximum int) (any, error) {
	lines, err := prepareYAMLLines(raw, maximum)
	if err != nil {
		return nil, err
	}
	parser := &yamlParser{lines: lines}
	parser.skipBlanks()
	value, err := parser.parseNode(0, 1)
	if err != nil {
		return nil, err
	}
	parser.skipBlanks()
	if parser.position != len(lines) {
		return nil, parser.errorf("trailing or incorrectly indented content")
	}
	return value, nil
}

func prepareYAMLLines(raw []byte, maximum int) ([]yamlLine, error) {
	if len(raw) == 0 || len(raw) > maximum || raw[len(raw)-1] != '\n' {
		return nil, fmt.Errorf("YAML source is empty, oversized, or lacks terminal LF")
	}
	if len(raw) == 1 || raw[len(raw)-2] == '\n' {
		return nil, fmt.Errorf("YAML source must have exactly one terminal LF")
	}
	for _, character := range raw {
		if character >= 0x80 || character == '\r' || character == '\t' || character < 0x20 && character != '\n' || character == 0x7f {
			return nil, fmt.Errorf("YAML source must be printable ASCII with LF only")
		}
	}
	physical := strings.Split(string(raw[:len(raw)-1]), "\n")
	lines := make([]yamlLine, len(physical))
	for index, text := range physical {
		line, err := prepareYAMLLine(text, index+1)
		if err != nil {
			return nil, err
		}
		lines[index] = line
	}
	return lines, nil
}

func prepareYAMLLine(raw string, number int) (yamlLine, error) {
	if strings.HasSuffix(raw, " ") {
		return yamlLine{}, fmt.Errorf("YAML line %d has trailing horizontal whitespace", number)
	}
	indent := 0
	for indent < len(raw) && raw[indent] == ' ' {
		indent++
	}
	if indent%2 != 0 {
		return yamlLine{}, fmt.Errorf("YAML line %d indentation is not a two-space step", number)
	}
	text := raw[indent:]
	if text == "---" || text == "..." || strings.HasPrefix(text, "%") {
		return yamlLine{}, fmt.Errorf("YAML line %d uses a directive or document marker", number)
	}
	return yamlLine{number: number, indent: indent, text: text, blank: text == ""}, nil
}

func (parser *yamlParser) parseNode(indent, depth int) (any, error) {
	if depth > maxYAMLDepth {
		return nil, parser.errorf("YAML depth exceeds %d", maxYAMLDepth)
	}
	parser.skipBlanks()
	if parser.position >= len(parser.lines) || parser.lines[parser.position].indent != indent {
		return nil, parser.errorf("expected content at indentation %d", indent)
	}
	if isSequenceLine(parser.lines[parser.position].text) {
		return parser.parseSequence(indent, depth)
	}
	return parser.parseMapping(indent, depth)
}

func (parser *yamlParser) parseMapping(indent, depth int) (map[string]any, error) {
	if err := parser.addCollection(depth); err != nil {
		return nil, err
	}
	result := make(map[string]any)
	for parser.position < len(parser.lines) {
		parser.skipBlanks()
		if parser.position >= len(parser.lines) || parser.lines[parser.position].indent < indent {
			break
		}
		line := parser.lines[parser.position]
		if line.indent != indent || isSequenceLine(line.text) {
			return nil, parser.errorf("unexpected mapping indentation or sequence item")
		}
		if len(result) >= maxYAMLFields {
			return nil, parser.errorf("YAML mapping exceeds %d fields", maxYAMLFields)
		}
		parser.position++
		if err := parser.parseMappingPair(result, line.text, indent, depth); err != nil {
			return nil, err
		}
	}
	if len(result) == 0 {
		return nil, parser.errorf("empty block mapping is unsupported")
	}
	return result, nil
}

func (parser *yamlParser) parseMappingPair(result map[string]any, text string, indent, depth int) error {
	if err := validateYAMLSyntax(text); err != nil {
		return parser.errorf("%v", err)
	}
	separator := findTopLevelColon(text)
	if separator <= 0 {
		return parser.errorf("mapping entry lacks a key separator")
	}
	if separator+1 < len(text) && (text[separator+1] != ' ' || separator+2 < len(text) && text[separator+2] == ' ') {
		return parser.errorf("mapping colon must be followed by exactly one space")
	}
	key, err := decodeYAMLKey(text[:separator])
	if err != nil || key == "<<" {
		return parser.errorf("invalid or forbidden mapping key %q", key)
	}
	if _, duplicate := result[key]; duplicate {
		return parser.errorf("duplicate mapping key %q", key)
	}
	if err := parser.addToken(); err != nil {
		return err
	}
	rest := text[separator+1:]
	if strings.HasPrefix(rest, " ") {
		rest = rest[1:]
	}
	value, err := parser.parseMappingValue(rest, indent, depth)
	if err != nil {
		return err
	}
	result[key] = value
	return nil
}

func (parser *yamlParser) parseMappingValue(rest string, indent, depth int) (any, error) {
	if depth+1 > maxYAMLDepth {
		return nil, parser.errorf("YAML depth exceeds %d", maxYAMLDepth)
	}
	if rest == ">-" {
		return parser.parseFoldedScalar(indent)
	}
	if rest == "" {
		parser.skipBlanks()
		if parser.position >= len(parser.lines) || parser.lines[parser.position].indent != indent+2 {
			return nil, parser.errorf("nested mapping value must be indented exactly two spaces")
		}
		return parser.parseNode(indent+2, depth+1)
	}
	if strings.HasPrefix(rest, "|") || strings.HasPrefix(rest, ">") {
		return nil, parser.errorf("only folded block scalar >- is supported")
	}
	return parser.parseInlineValue(rest, depth+1)
}

func (parser *yamlParser) parseFoldedScalar(indent int) (any, error) {
	if parser.position >= len(parser.lines) || parser.lines[parser.position].blank {
		return nil, parser.errorf("folded scalar requires adjacent ordinary content")
	}
	parts := make([]string, 0)
	valueBytes := 0
	for parser.position < len(parser.lines) {
		line := parser.lines[parser.position]
		if line.blank || line.indent <= indent {
			break
		}
		if line.indent != indent+2 || line.text == "" {
			return nil, parser.errorf("folded scalar content indentation must be exactly two spaces")
		}
		if err := validateFoldedContent(line.text); err != nil {
			return nil, parser.errorf("folded scalar content: %v", err)
		}
		if len(parts) > 0 {
			valueBytes++
		}
		valueBytes += len(line.text)
		if valueBytes > maxYAMLScalarBytes {
			return nil, parser.errorf("folded scalar is oversized")
		}
		parts = append(parts, line.text)
		parser.position++
	}
	value := strings.Join(parts, " ")
	if len(parts) == 0 {
		return nil, parser.errorf("folded scalar is empty or oversized")
	}
	if err := parser.addToken(); err != nil {
		return nil, err
	}
	return value, nil
}

func validateFoldedContent(text string) error {
	if strings.ContainsAny(text, "#&*!'\\") {
		return fmt.Errorf("YAML indicator, single quote, or backslash byte is forbidden in folded content")
	}
	if strings.Count(text, `"`)%2 != 0 {
		return fmt.Errorf("folded content contains an unmatched double quote")
	}
	if text == "---" || text == "..." || strings.HasPrefix(text, "%") || strings.HasPrefix(text, "<<:") {
		return fmt.Errorf("directive, document marker, or merge syntax is forbidden in folded content")
	}
	return nil
}

func (parser *yamlParser) parseSequence(indent, depth int) ([]any, error) {
	if err := parser.addCollection(depth); err != nil {
		return nil, err
	}
	result := make([]any, 0)
	for parser.position < len(parser.lines) {
		parser.skipBlanks()
		if parser.position >= len(parser.lines) || parser.lines[parser.position].indent < indent {
			break
		}
		line := parser.lines[parser.position]
		if line.indent != indent || !isSequenceLine(line.text) {
			return nil, parser.errorf("unexpected sequence indentation or mapping entry")
		}
		if len(result) >= maxYAMLItems {
			return nil, parser.errorf("YAML sequence exceeds %d items", maxYAMLItems)
		}
		parser.position++
		item, err := parser.parseSequenceItem(line, indent, depth)
		if err != nil {
			return nil, err
		}
		result = append(result, item)
	}
	if len(result) == 0 {
		return nil, parser.errorf("empty block sequence is unsupported")
	}
	return result, nil
}

func (parser *yamlParser) parseSequenceItem(line yamlLine, indent, depth int) (any, error) {
	if line.text == "-" {
		parser.skipBlanks()
		if parser.position >= len(parser.lines) || parser.lines[parser.position].indent != indent+2 {
			return nil, parser.errorf("bare sequence item requires an exactly indented child")
		}
		return parser.parseNode(indent+2, depth+1)
	}
	rest := strings.TrimPrefix(line.text, "- ")
	if rest == "" || rest[0] == ' ' {
		return nil, parser.errorf("sequence item has empty or padded inline content")
	}
	if err := validateYAMLSyntax(rest); err != nil {
		return nil, parser.errorf("%v", err)
	}
	if rest[0] != '[' && rest[0] != '{' && hasOutsideQuotedColon(rest) {
		return parser.parseCompactMapping(rest, indent, depth+1)
	}
	return parser.parseInlineValue(rest, depth+1)
}

func hasOutsideQuotedColon(text string) bool {
	quoted, square, curly := false, 0, 0
	for index := 0; index < len(text); index++ {
		switch text[index] {
		case '"':
			quoted = !quoted
		case '[':
			if !quoted {
				square++
			}
		case ']':
			if !quoted && square > 0 {
				square--
			}
		case '{':
			if !quoted {
				curly++
			}
		case '}':
			if !quoted && curly > 0 {
				curly--
			}
		case ':':
			if !quoted && square == 0 && curly == 0 {
				return true
			}
		}
	}
	return false
}

func (parser *yamlParser) parseCompactMapping(first string, sequenceIndent, depth int) (map[string]any, error) {
	if err := parser.addCollection(depth); err != nil {
		return nil, err
	}
	result := make(map[string]any)
	if err := parser.parseMappingPair(result, first, sequenceIndent+2, depth); err != nil {
		return nil, err
	}
	for parser.position < len(parser.lines) {
		parser.skipBlanks()
		if parser.position >= len(parser.lines) || parser.lines[parser.position].indent <= sequenceIndent {
			break
		}
		line := parser.lines[parser.position]
		if line.indent != sequenceIndent+2 || isSequenceLine(line.text) || len(result) >= maxYAMLFields {
			return nil, parser.errorf("invalid compact mapping continuation")
		}
		parser.position++
		if err := parser.parseMappingPair(result, line.text, line.indent, depth); err != nil {
			return nil, err
		}
	}
	return result, nil
}

func isSequenceLine(text string) bool {
	return text == "-" || strings.HasPrefix(text, "- ")
}

func findTopLevelColon(text string) int {
	quoted, square, curly := false, 0, 0
	for index := 0; index < len(text); index++ {
		switch text[index] {
		case '"':
			quoted = !quoted
		case '[':
			if !quoted {
				square++
			}
		case ']':
			if !quoted && square > 0 {
				square--
			}
		case '{':
			if !quoted {
				curly++
			}
		case '}':
			if !quoted && curly > 0 {
				curly--
			}
		case ':':
			if !quoted && square == 0 && curly == 0 && (index+1 == len(text) || text[index+1] == ' ') {
				return index
			}
		}
	}
	return -1
}

func validateYAMLSyntax(text string) error {
	quoted := false
	for index := 0; index < len(text); index++ {
		character := text[index]
		if character == '"' {
			quoted = !quoted
			continue
		}
		if !quoted && (character == '\\' || character == '\'') {
			return fmt.Errorf("single quotes and backslash bytes are forbidden outside double quotes")
		}
		if quoted && character == '\\' {
			return fmt.Errorf("forbidden YAML syntax byte %q", character)
		}
		if !quoted && character == '#' {
			return fmt.Errorf("YAML comment syntax is forbidden")
		}
		if !quoted && strings.ContainsRune("&*!", rune(character)) {
			return fmt.Errorf("YAML anchor, alias, or tag syntax is forbidden")
		}
	}
	if quoted {
		return fmt.Errorf("unterminated double-quoted YAML scalar")
	}
	return nil
}

func decodeYAMLKey(value string) (string, error) {
	if len(value) >= 2 && value[0] == '"' && value[len(value)-1] == '"' {
		value = value[1 : len(value)-1]
	} else if strings.ContainsRune(value, '"') {
		return "", fmt.Errorf("invalid YAML key quoting")
	}
	if !validYAMLKey(value) {
		return "", fmt.Errorf("invalid YAML key")
	}
	return value, nil
}

func validYAMLKey(value string) bool {
	if value == "" || len(value) > maxYAMLScalarBytes {
		return false
	}
	if first := value[0]; (first < 'a' || first > 'z') && (first < 'A' || first > 'Z') && (first < '0' || first > '9') {
		return false
	}
	for _, character := range []byte(value[1:]) {
		if (character < 'a' || character > 'z') && (character < 'A' || character > 'Z') &&
			(character < '0' || character > '9') && character != '_' && character != '-' && character != '.' && character != '/' {
			return false
		}
	}
	return true
}

func (parser *yamlParser) parsePlainScalar(value string) (any, error) {
	value = strings.TrimSpace(value)
	if value == "" || len(value) > maxYAMLScalarBytes || strings.ContainsAny(value, "\"'") {
		return nil, parser.errorf("plain scalar is empty, oversized, or malformed")
	}
	if value == "true" || value == "false" {
		return value == "true", parser.addToken()
	}
	if value == "null" {
		return nil, parser.addToken()
	}
	if isCanonicalInteger(value) {
		integer, _ := strconv.ParseInt(value, 10, 64)
		return integer, parser.addToken()
	}
	if strings.ContainsRune("-?:,[]{}#&*!|>'%@`", rune(value[0])) || strings.HasSuffix(value, ":") || strings.ContainsRune(value, '\\') {
		return nil, parser.errorf("plain scalar uses a forbidden YAML indicator")
	}
	if looksLikeForbiddenTypedScalar(value) {
		return nil, parser.errorf("unsupported or noncanonical typed scalar %q", value)
	}
	if err := parser.addToken(); err != nil {
		return nil, err
	}
	return value, nil
}

func looksLikeForbiddenTypedScalar(value string) bool {
	lower := strings.ToLower(value)
	for _, keyword := range []string{
		"~", "y", "yes", "n", "no", "on", "off", "null", "true", "false", ".inf", "+.inf", "-.inf",
		"inf", "+inf", "-inf", "infinity", "+infinity", "-infinity", ".nan", "+.nan", "-.nan", "nan", "+nan", "-nan",
	} {
		if lower == keyword && value != "null" && value != "true" && value != "false" {
			return true
		}
	}
	return numericLikePattern.MatchString(value) || timestampPattern.MatchString(value)
}

func isCanonicalInteger(value string) bool {
	parsed, err := strconv.ParseInt(value, 10, 64)
	return err == nil && strconv.FormatInt(parsed, 10) == value
}

func (parser *yamlParser) addCollection(depth int) error {
	if depth > maxYAMLDepth || parser.collections >= maxYAMLCollections {
		return parser.errorf("YAML collection depth or count exceeded")
	}
	parser.collections++
	return parser.addToken()
}

func (parser *yamlParser) addToken() error {
	if parser.tokens >= maxYAMLTokens {
		return parser.errorf("YAML token count exceeds %d", maxYAMLTokens)
	}
	parser.tokens++
	return nil
}

func (parser *yamlParser) skipBlanks() {
	for parser.position < len(parser.lines) && parser.lines[parser.position].blank {
		parser.position++
	}
}

func (parser *yamlParser) errorf(format string, arguments ...any) error {
	line := len(parser.lines)
	if parser.position < len(parser.lines) {
		line = parser.lines[parser.position].number
	}
	return fmt.Errorf("YAML line %d: %s", line, fmt.Sprintf(format, arguments...))
}
