package graphdispatch

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestCommandEmitsExactGoldenContractFromStdin(t *testing.T) {
	fixture := readSharedFixture(t)
	var stdout, stderr bytes.Buffer
	code := Command(
		validCommandArgs("-"),
		strings.NewReader(fixture.Input.CanonicalControlSnapshotJSON),
		&stdout,
		&stderr,
	)
	if code != 0 || stderr.Len() != 0 {
		t.Fatalf("Command code=%d stderr=%q", code, stderr.String())
	}
	if stdout.String() != fixture.Expected.CanonicalContractJSON ||
		bytes.HasSuffix(stdout.Bytes(), []byte{'\n'}) {
		t.Fatalf("stdout is not exact canonical contract: %q", stdout.Bytes())
	}
}

func TestCommandReadsOnlyExplicitControlFile(t *testing.T) {
	fixture := readSharedFixture(t)
	path := filepath.Join(t.TempDir(), "private-control.json")
	if err := os.WriteFile(
		path,
		[]byte(fixture.Input.CanonicalControlSnapshotJSON),
		0o600,
	); err != nil {
		t.Fatalf("write fixture: %v", err)
	}
	var stdout, stderr bytes.Buffer
	code := Command(validCommandArgs(path), strings.NewReader("ignored"), &stdout, &stderr)
	if code != 0 || stderr.Len() != 0 ||
		stdout.String() != fixture.Expected.CanonicalContractJSON {
		t.Fatalf("Command code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
	}
}

func TestCommandRejectsArgumentsWithoutDisclosingValues(t *testing.T) {
	secret := "TOP-SECRET-ENDPOINT-OR-MODEL"
	tests := invalidCommandCases(secret)
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			var stdout, stderr bytes.Buffer
			code := Command(test.args, strings.NewReader(test.input), &stdout, &stderr)
			if code != test.code || stdout.Len() != 0 {
				t.Fatalf("code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
			}
			if strings.Contains(stderr.String(), secret) {
				t.Fatalf("stderr disclosed caller input: %q", stderr.String())
			}
		})
	}
}

func TestCommandHandlesHelpAndOutputFailure(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if code := Command([]string{"--help"}, strings.NewReader(""), &stdout, &stderr); code != 0 ||
		!strings.Contains(stderr.String(), "forge graph-node-contract") {
		t.Fatalf("help code/output = %d / %q", code, stderr.String())
	}
	fixture := readSharedFixture(t)
	stderr.Reset()
	code := Command(
		validCommandArgs("-"),
		strings.NewReader(fixture.Input.CanonicalControlSnapshotJSON),
		shortWriter{},
		&stderr,
	)
	if code != 1 || !strings.Contains(stderr.String(), "cannot write") {
		t.Fatalf("short write code=%d stderr=%q", code, stderr.String())
	}
}

type invalidCommandCase struct {
	name  string
	args  []string
	input string
	code  int
}

func invalidCommandCases(secret string) []invalidCommandCase {
	valid := validCommandArgs("-")
	return []invalidCommandCase{
		{"missing flags", []string{"--control", "-"}, "", 2},
		{"positional", append(valid, secret), "", 2},
		{"duplicate", append(valid, "--model", secret), "", 2},
		{"negative", replaceFlagValue(valid, "--timeout-ms", "-1"), "", 2},
		{"overflow", replaceFlagValue(valid, "--timeout-ms", "18446744073709551616"), "", 2},
		{"zero", replaceFlagValue(valid, "--max-cost-usd-micros", "0"), "", 2},
		{"high", replaceFlagValue(valid, "--max-result-bytes", "524289"), "", 2},
		{"invalid endpoint", replaceFlagValue(valid, "--endpoint", "https://"+secret+"@api.example/v1"), "", 2},
		{"invalid model", replaceFlagValue(valid, "--model", secret+"\n"), "", 2},
		{"invalid pricing", replaceFlagValue(valid, "--pricing-snapshot-sha256", secret), "", 2},
		{"invalid control", valid, `{"opaque":"TOP-SECRET-ENDPOINT-OR-MODEL"}`, 1},
	}
}

func validCommandArgs(control string) []string {
	return []string{
		"--control", control,
		"--endpoint", "https://api.openai.com/v1/responses",
		"--model", "gpt-5.6-sol",
		"--max-output-tokens", "4096",
		"--max-model-output-bytes", "65536",
		"--max-model-events", "4096",
		"--timeout-ms", "60000",
		"--max-cost-usd-micros", "1000000",
		"--pricing-snapshot-sha256", strings.Repeat("4", 64),
		"--max-result-bytes", "524288",
	}
}

func replaceFlagValue(args []string, name, value string) []string {
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
		return 0, errors.New("empty")
	}
	return len(data) - 1, nil
}
