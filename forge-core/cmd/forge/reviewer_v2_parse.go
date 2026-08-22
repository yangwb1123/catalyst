package main

import (
	"strings"
	"unicode"
	"unicode/utf8"
)

const (
	reviewerBindingToken  = "REVIEW_BINDING_SHA256:"
	reviewerBindingPrefix = reviewerBindingToken + " "
)

func parseReviewerV2Verdict(payload, expectedBinding string) (string, bool) {
	if !utf8.ValidString(payload) || strings.Contains(payload, "\r") || hasForbiddenControl(payload) {
		return "", false
	}
	lines := strings.Split(payload, "\n")
	last := len(lines) - 1
	for last >= 0 && lines[last] == "" {
		last--
	}
	if last < 1 || lines[last-1] != reviewerBindingPrefix+expectedBinding {
		return "", false
	}
	verdict, ok := exactReviewerVerdictLine(lines[last])
	if !ok || countContainingLines(lines, reviewerBindingToken) != 1 ||
		countExactLine(lines, reviewerBindingPrefix+expectedBinding) != 1 ||
		countVerdictLines(lines) != 1 {
		return "", false
	}
	return verdict, true
}

func countContainingLines(lines []string, token string) int {
	count := 0
	for _, line := range lines {
		if strings.Contains(line, token) {
			count++
		}
	}
	return count
}

func exactReviewerVerdictLine(line string) (string, bool) {
	switch line {
	case "VERDICT: APPROVE":
		return VerdictApprove, true
	case "VERDICT: REQUEST_CHANGES":
		return VerdictRequestChanges, true
	default:
		return "", false
	}
}

func countExactLine(lines []string, want string) int {
	count := 0
	for _, line := range lines {
		if line == want {
			count++
		}
	}
	return count
}

func countVerdictLines(lines []string) int {
	count := 0
	for _, line := range lines {
		if strings.Contains(line, "VERDICT:") {
			count++
		}
	}
	return count
}

func hasForbiddenControl(value string) bool {
	for _, character := range value {
		if unicode.IsControl(character) && character != '\n' && character != '\t' {
			return true
		}
	}
	return false
}
