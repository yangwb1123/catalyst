package graphpricing

import (
	"bytes"
	"errors"
	"strings"
	"testing"
)

func TestCommandEmitsExactGoldenSnapshotWithoutTrailingLF(t *testing.T) {
	fixture := readSharedFixture(t)
	var stdout, stderr bytes.Buffer
	code := Command(fixtureArgs(), &stdout, &stderr)
	if code != 0 || stderr.Len() != 0 ||
		stdout.String() != fixture.Expected.CanonicalPricingSnapshotJSON {
		t.Fatalf("Command code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
	}
	if bytes.HasSuffix(stdout.Bytes(), []byte{'\n'}) {
		t.Fatal("pricing snapshot has a trailing LF")
	}
}

func TestCommandRejectsArgumentDriftWithoutEchoingValues(t *testing.T) {
	secret := "TOP-SECRET-PRICING-VALUE"
	valid := fixtureArgs()
	tests := [][]string{
		nil,
		{"--model", "gpt-5.6-sol"},
		append(append([]string(nil), valid...), secret),
		append(append([]string(nil), valid...), "--model", secret),
		replaceFlag(valid, "--input-usd-micros-per-token-unit", "0"),
		replaceFlag(valid, "--output-usd-micros-per-token-unit", "1000000000001"),
		replaceFlag(valid, "--max-input-tokens", "-1"),
		replaceFlag(valid, "--max-input-tokens", "18446744073709551616"),
		replaceFlag(valid, "--model", secret+"\n"),
		append(append([]string(nil), valid...), "--endpoint", secret),
		append(append([]string(nil), valid...), "--provider", secret),
	}
	for _, args := range tests {
		var stdout, stderr bytes.Buffer
		if code := Command(args, &stdout, &stderr); code != 2 || stdout.Len() != 0 {
			t.Fatalf("args=%v code=%d stdout=%q stderr=%q", args, code, stdout.String(), stderr.String())
		}
		if strings.Contains(stderr.String(), secret) {
			t.Fatalf("stderr disclosed caller value: %q", stderr.String())
		}
	}
}

func TestCommandHelpStatesEffectFreeBoundaryAndHandlesOutputFailure(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if code := Command([]string{"--help"}, &stdout, &stderr); code != 0 || stdout.Len() != 0 ||
		!strings.Contains(stderr.String(), "operator-asserted local pricing artifact") ||
		!strings.Contains(stderr.String(), "performs no network request") {
		t.Fatalf("help code/output = %d / %q / %q", code, stdout.String(), stderr.String())
	}
	stderr.Reset()
	if code := Command(fixtureArgs(), shortWriter{}, &stderr); code != 1 ||
		!strings.Contains(stderr.String(), "cannot write") {
		t.Fatalf("short write code/output = %d / %q", code, stderr.String())
	}
}

func fixtureArgs() []string {
	return []string{
		"--model", "gpt-5.6-sol",
		"--input-usd-micros-per-token-unit", "2000000",
		"--output-usd-micros-per-token-unit", "10000000",
		"--max-input-tokens", "400000",
	}
}

func replaceFlag(args []string, name, value string) []string {
	result := append([]string(nil), args...)
	for index := range result {
		if result[index] == name && index+1 < len(result) {
			result[index+1] = value
			return result
		}
	}
	return result
}

type shortWriter struct{}

func (shortWriter) Write(data []byte) (int, error) {
	if len(data) == 0 {
		return 0, errors.New("empty output")
	}
	return len(data) - 1, nil
}
