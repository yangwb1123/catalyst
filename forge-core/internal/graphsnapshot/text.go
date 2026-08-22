package graphsnapshot

import (
	"fmt"
	"sort"
	"strings"
	"unicode"
	"unicode/utf8"
)

func validIdentifier(value string) bool {
	if value == "" || len(value) > 160 || !utf8.ValidString(value) {
		return false
	}
	firstLetter := value[0] >= 'a' && value[0] <= 'z'
	firstDigit := value[0] >= '0' && value[0] <= '9'
	if !firstLetter && !firstDigit {
		return false
	}
	for _, character := range []byte(value) {
		letter := character >= 'a' && character <= 'z'
		digit := character >= '0' && character <= '9'
		if !letter && !digit && !strings.ContainsRune("._:/-", rune(character)) {
			return false
		}
	}
	return true
}

func validBoundedText(value string) bool {
	if value == "" || len(value) > 16_384 || utf8.RuneCountInString(value) > 4_096 || !utf8.ValidString(value) {
		return false
	}
	for _, character := range value {
		if unicode.Is(unicode.Cc, character) || forbiddenDirectional(character) {
			return false
		}
	}
	return true
}

func forbiddenDirectional(value rune) bool {
	return value == 0x061c || value == 0x200e || value == 0x200f ||
		value == 0x2028 || value == 0x2029 || value >= 0x202a && value <= 0x202e ||
		value >= 0x2066 && value <= 0x2069
}

func moduleRelative(value, root string) (string, error) {
	if value == root {
		return ".", nil
	}
	if root == "." {
		if value == "." || !validBoundedText(value) {
			return "", fmt.Errorf("value is outside root module")
		}
		return value, nil
	}
	prefix := root + "/"
	if !strings.HasPrefix(value, prefix) {
		return "", fmt.Errorf("value is outside selected module")
	}
	result := strings.TrimPrefix(value, prefix)
	if !validBoundedText(result) {
		return "", fmt.Errorf("module-relative value is invalid")
	}
	return result, nil
}

func splitPipe(value string) []string { return strings.Split(value, "|") }

func uniqueSorted(values []string) []string {
	result := append([]string{}, values...)
	sort.Strings(result)
	if len(result) < 2 {
		return result
	}
	write := 1
	for _, value := range result[1:] {
		if value != result[write-1] {
			result[write], write = value, write+1
		}
	}
	return result[:write]
}

func stringCopy(value *string) *string {
	if value == nil {
		return nil
	}
	result := *value
	return &result
}
