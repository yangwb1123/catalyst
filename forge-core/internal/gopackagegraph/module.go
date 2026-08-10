package gopackagegraph

import (
	"fmt"
	"strconv"
	"strings"
)

func parseModuleDirective(content string) (string, error) {
	content = strings.TrimPrefix(content, "\ufeff")
	modulePath := ""
	lexer := moduleDirectiveLexer{}
	for index, line := range strings.Split(content, "\n") {
		line, topLevel, err := lexer.scanLine(line)
		if err != nil {
			return "", fmt.Errorf("line %d: %w", index+1, err)
		}
		line = strings.TrimSpace(line)
		if !topLevel || line == "" || !directivePrefix(line, "module") {
			continue
		}
		if modulePath != "" {
			return "", fmt.Errorf("multiple module directives")
		}
		value := strings.TrimSpace(strings.TrimPrefix(line, "module"))
		parsed, err := parseModulePathValue(value)
		if err != nil {
			return "", err
		}
		modulePath = parsed
	}
	if lexer.depth != 0 {
		return "", fmt.Errorf("unterminated directive block")
	}
	if modulePath == "" {
		return "", fmt.Errorf("module directive is absent")
	}
	return modulePath, nil
}

type moduleDirectiveLexer struct {
	depth int
}

func (lexer *moduleDirectiveLexer) scanLine(line string) (string, bool, error) {
	topLevel := lexer.depth == 0
	quote, escaped := byte(0), false
	for index := 0; index < len(line); index++ {
		character := line[index]
		if quote != 0 {
			if quote == '"' && escaped {
				escaped = false
				continue
			}
			if quote == '"' && character == '\\' {
				escaped = true
				continue
			}
			if character == quote {
				quote = 0
			}
			continue
		}
		if character == '/' && index+1 < len(line) && line[index+1] == '/' {
			return line[:index], topLevel, nil
		}
		if index+1 < len(line) &&
			((character == '/' && line[index+1] == '*') ||
				(character == '*' && line[index+1] == '/')) {
			return "", false, fmt.Errorf("block comments are not permitted")
		}
		switch character {
		case '"', '`':
			quote = character
		case '(':
			if lexer.depth != 0 {
				return "", false, fmt.Errorf("nested directive block")
			}
			lexer.depth++
		case ')':
			if lexer.depth == 0 {
				return "", false, fmt.Errorf("unexpected closing parenthesis")
			}
			lexer.depth--
		}
	}
	if quote != 0 {
		return "", false, fmt.Errorf("unterminated quoted token")
	}
	return line, topLevel, nil
}

func directivePrefix(line, directive string) bool {
	if !strings.HasPrefix(line, directive) || len(line) == len(directive) {
		return false
	}
	next := line[len(directive)]
	return next == ' ' || next == '\t'
}

func parseModulePathValue(value string) (string, error) {
	if value == "" {
		return "", fmt.Errorf("module directive has no path")
	}
	modulePath := value
	if value[0] == '`' || value[0] == '"' {
		unquoted, err := strconv.Unquote(value)
		if err != nil {
			return "", fmt.Errorf("module directive has invalid quoted path")
		}
		modulePath = unquoted
	} else if len(strings.Fields(value)) != 1 {
		return "", fmt.Errorf("module directive has trailing tokens")
	}
	if !canonicalImportPath(modulePath) {
		return "", fmt.Errorf("module directive path is not canonical")
	}
	return modulePath, nil
}
