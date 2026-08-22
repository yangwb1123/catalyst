package adrv2

import (
	"fmt"
	"strings"
)

var bodyHeadings = []string{
	"Context", "Decision", "Consequences", "Validation", "Limitations",
}

func validateBody(body []byte, adrID, title string) error {
	text := string(body)
	if !strings.HasSuffix(text, "\n") || strings.HasSuffix(text, "\n\n") {
		return fmt.Errorf("ADR body must end with exactly one LF")
	}
	for _, character := range text {
		if character != '\n' && forbiddenRune(character) {
			return fmt.Errorf("ADR body contains forbidden Unicode U+%04X", character)
		}
	}
	for _, line := range strings.Split(text, "\n") {
		if strings.HasSuffix(line, " ") || strings.HasSuffix(line, "\t") {
			return fmt.Errorf("ADR body lines must not contain trailing whitespace")
		}
	}
	prefix := "# " + adrID + ": " + title + "\n\n"
	if !strings.HasPrefix(text, prefix) {
		return fmt.Errorf("ADR body heading must be %q followed by one blank line", strings.TrimSuffix(prefix, "\n\n"))
	}
	return validateBodySections(text[len(prefix):])
}

func validateBodySections(remaining string) error {
	sections := make([]string, 0, len(bodyHeadings))
	for index, heading := range bodyHeadings {
		prefix := "## " + heading + "\n"
		if !strings.HasPrefix(remaining, prefix) {
			return fmt.Errorf("ADR body requires exact section %q in frozen order", heading)
		}
		remaining = remaining[len(prefix):]
		content := ""
		if index+1 < len(bodyHeadings) {
			marker := "\n\n## " + bodyHeadings[index+1] + "\n"
			boundary := strings.Index(remaining, marker)
			if boundary < 0 {
				return fmt.Errorf("ADR body cannot find section after %q", heading)
			}
			content, remaining = remaining[:boundary], remaining[boundary+2:]
		} else {
			content, remaining = strings.TrimSuffix(remaining, "\n"), ""
		}
		if content == "" || strings.TrimSpace(content) != content {
			return fmt.Errorf("ADR body section %q must contain canonical nonempty text", heading)
		}
		sections = append(sections, content)
	}
	if remaining != "" {
		return fmt.Errorf("ADR body contains trailing data")
	}
	for _, section := range sections {
		if containsLevelTwoHeading(section) {
			return fmt.Errorf("ADR body contains an extra level-two section")
		}
	}
	return nil
}

func containsLevelTwoHeading(section string) bool {
	lines := strings.Split(section, "\n")
	for index, line := range lines {
		candidate := strings.TrimLeft(line, " ")
		indent := len(line) - len(candidate)
		if indent <= 3 && (candidate == "##" || strings.HasPrefix(candidate, "## ")) {
			return true
		}
		if index > 0 && strings.TrimSpace(lines[index-1]) != "" &&
			indent <= 3 && isSetextLevelTwo(candidate) {
			return true
		}
	}
	return false
}

func isSetextLevelTwo(line string) bool {
	if line == "" {
		return false
	}
	for _, character := range line {
		if character != '-' {
			return false
		}
	}
	return true
}
