package planningownership

import "strings"

type flowCursor struct {
	text   string
	index  int
	parser *yamlParser
}

func (parser *yamlParser) parseInlineValue(text string, depth int) (any, error) {
	if depth > maxYAMLDepth {
		return nil, parser.errorf("YAML depth exceeds %d", maxYAMLDepth)
	}
	if text[0] != '[' && text[0] != '{' && text[0] != '"' {
		return parser.parsePlainScalar(text)
	}
	cursor := &flowCursor{text: text, parser: parser}
	value, err := cursor.parseValue(depth)
	if err != nil {
		return nil, err
	}
	cursor.skipSpaces()
	if cursor.index != len(cursor.text) {
		return nil, parser.errorf("trailing inline YAML content")
	}
	return value, nil
}

func (cursor *flowCursor) parseValue(depth int) (any, error) {
	if depth > maxYAMLDepth {
		return nil, cursor.parser.errorf("YAML depth exceeds %d", maxYAMLDepth)
	}
	cursor.skipSpaces()
	if cursor.index >= len(cursor.text) {
		return nil, cursor.parser.errorf("missing inline YAML value")
	}
	switch cursor.text[cursor.index] {
	case '[':
		return cursor.parseSequence(depth)
	case '{':
		return cursor.parseMapping(depth)
	case '"':
		return cursor.parseQuoted()
	default:
		return cursor.parsePlain()
	}
}

func (cursor *flowCursor) parseSequence(depth int) ([]any, error) {
	if err := cursor.parser.addCollection(depth); err != nil {
		return nil, err
	}
	cursor.index++
	cursor.skipSpaces()
	result := make([]any, 0)
	if cursor.consume(']') {
		return result, nil
	}
	for {
		if len(result) >= maxYAMLItems {
			return nil, cursor.parser.errorf("flow sequence exceeds %d items", maxYAMLItems)
		}
		value, err := cursor.parseValue(depth + 1)
		if err != nil {
			return nil, err
		}
		result = append(result, value)
		cursor.skipSpaces()
		if cursor.consume(']') {
			return result, nil
		}
		if !cursor.consume(',') {
			return nil, cursor.parser.errorf("flow sequence requires comma or closing bracket")
		}
		cursor.skipSpaces()
		if cursor.peek(']') {
			return nil, cursor.parser.errorf("flow sequence trailing comma is forbidden")
		}
	}
}

func (cursor *flowCursor) parseMapping(depth int) (map[string]any, error) {
	if err := cursor.parser.addCollection(depth); err != nil {
		return nil, err
	}
	cursor.index++
	cursor.skipSpaces()
	result := make(map[string]any)
	if cursor.consume('}') {
		return result, nil
	}
	for {
		if len(result) >= maxYAMLFields {
			return nil, cursor.parser.errorf("flow mapping exceeds %d fields", maxYAMLFields)
		}
		if err := cursor.parseMappingEntry(result, depth); err != nil {
			return nil, err
		}
		cursor.skipSpaces()
		if cursor.consume('}') {
			return result, nil
		}
		if !cursor.consume(',') {
			return nil, cursor.parser.errorf("flow mapping requires comma or closing brace")
		}
		cursor.skipSpaces()
		if cursor.peek('}') {
			return nil, cursor.parser.errorf("flow mapping trailing comma is forbidden")
		}
	}
}

func (cursor *flowCursor) parseMappingEntry(result map[string]any, depth int) error {
	key, err := cursor.parseKey()
	if err != nil {
		return err
	}
	if !cursor.consume(':') {
		return cursor.parser.errorf("flow mapping key lacks colon")
	}
	if cursor.index >= len(cursor.text) || cursor.text[cursor.index] != ' ' ||
		cursor.index+1 >= len(cursor.text) || cursor.text[cursor.index+1] == ' ' {
		return cursor.parser.errorf("flow mapping colon must be followed by exactly one space")
	}
	cursor.index++
	if _, duplicate := result[key]; duplicate {
		return cursor.parser.errorf("duplicate flow mapping key %q", key)
	}
	if err := cursor.parser.addToken(); err != nil {
		return err
	}
	value, err := cursor.parseValue(depth + 1)
	if err != nil {
		return err
	}
	result[key] = value
	return nil
}

func (cursor *flowCursor) parseKey() (string, error) {
	cursor.skipSpaces()
	if cursor.peek('"') {
		key, err := cursor.parseQuotedText()
		if err != nil {
			return "", err
		}
		if !validYAMLKey(key) || key == "<<" {
			return "", cursor.parser.errorf("invalid or forbidden flow mapping key %q", key)
		}
		return key, nil
	}
	start := cursor.index
	for cursor.index < len(cursor.text) && cursor.text[cursor.index] != ':' {
		if strings.ContainsRune("{},[]", rune(cursor.text[cursor.index])) {
			return "", cursor.parser.errorf("flow mapping key is malformed")
		}
		cursor.index++
	}
	key := cursor.text[start:cursor.index]
	if !validYAMLKey(key) || key == "<<" {
		return "", cursor.parser.errorf("invalid or forbidden flow mapping key %q", key)
	}
	return key, nil
}

func (cursor *flowCursor) parseQuoted() (any, error) {
	value, err := cursor.parseQuotedText()
	if err != nil {
		return nil, err
	}
	if err := cursor.parser.addToken(); err != nil {
		return nil, err
	}
	return value, nil
}

func (cursor *flowCursor) parseQuotedText() (string, error) {
	cursor.index++
	start := cursor.index
	for cursor.index < len(cursor.text) && cursor.text[cursor.index] != '"' {
		if cursor.text[cursor.index] == '\\' {
			return "", cursor.parser.errorf("escape bytes are forbidden in double-quoted YAML")
		}
		cursor.index++
	}
	if cursor.index >= len(cursor.text) {
		return "", cursor.parser.errorf("unterminated double-quoted YAML scalar")
	}
	value := cursor.text[start:cursor.index]
	cursor.index++
	if len(value) > maxYAMLScalarBytes {
		return "", cursor.parser.errorf("quoted scalar exceeds %d bytes", maxYAMLScalarBytes)
	}
	return value, nil
}

func (cursor *flowCursor) parsePlain() (any, error) {
	start := cursor.index
	for cursor.index < len(cursor.text) && !strings.ContainsRune(",]}", rune(cursor.text[cursor.index])) {
		cursor.index++
	}
	return cursor.parser.parsePlainScalar(cursor.text[start:cursor.index])
}

func (cursor *flowCursor) skipSpaces() {
	for cursor.index < len(cursor.text) && cursor.text[cursor.index] == ' ' {
		cursor.index++
	}
}

func (cursor *flowCursor) consume(expected byte) bool {
	if cursor.index >= len(cursor.text) || cursor.text[cursor.index] != expected {
		return false
	}
	cursor.index++
	return true
}

func (cursor *flowCursor) peek(expected byte) bool {
	return cursor.index < len(cursor.text) && cursor.text[cursor.index] == expected
}
