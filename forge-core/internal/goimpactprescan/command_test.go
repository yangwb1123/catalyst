package goimpactprescan

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestCommandEmitsExactEnvelopeFromStdin(t *testing.T) {
	graph := marshalGraph(richObservation())
	args := commandArgs(graphDigest(graph), "impact-fixture-001", richChangedPaths)
	var stdout, stderr bytes.Buffer
	code := Command(args, bytes.NewReader(graph), &stdout, &stderr)
	if code != 0 || stderr.Len() != 0 {
		t.Fatalf("Command = %d, stderr=%q", code, stderr.String())
	}
	if _, err := Decode(stdout.Bytes()); err != nil {
		t.Fatalf("command output: %v", err)
	}
	if bytes.HasSuffix(stdout.Bytes(), []byte{'\n'}) {
		t.Fatal("canonical output has trailing LF")
	}
}

func TestCommandReadsOnlyExplicitGraphFile(t *testing.T) {
	graph := marshalGraph(richObservation())
	path := filepath.Join(t.TempDir(), "graph.json")
	if err := os.WriteFile(path, graph, 0o600); err != nil {
		t.Fatal(err)
	}
	args := append(commandArgs(graphDigest(graph), "impact-fixture-001", richChangedPaths),
		"--input", path)
	var stdout, stderr bytes.Buffer
	if code := Command(args, bytes.NewReader(nil), &stdout, &stderr); code != 0 ||
		stdout.Len() == 0 || stderr.Len() != 0 {
		t.Fatalf("Command = %d/%q/%q", code, stdout.String(), stderr.String())
	}
}

func TestCommandRejectsDriftWithoutDisclosingInput(t *testing.T) {
	secret := "TOP-SECRET-GRAPH-BODY"
	graph := marshalGraph(richObservation())
	tests := []struct {
		args  []string
		input []byte
		code  int
	}{
		{args: nil, code: 2},
		{args: append(commandArgs(graphDigest(graph), "impact-fixture-001", richChangedPaths), secret), input: graph, code: 2},
		{args: append(commandArgs(graphDigest(graph), "impact-fixture-001", richChangedPaths), "--run-id", secret), input: graph, code: 2},
		{args: commandArgs(strings.Repeat("0", 64), "impact-fixture-001", richChangedPaths), input: graph, code: 1},
		{args: commandArgs(graphDigest(graph), "impact-fixture-001", richChangedPaths), input: []byte(secret), code: 1},
		{args: commandArgs(graphDigest(graph), "impact-fixture-001", []string{"service/z/z.go", "service/d/d.go"}), input: graph, code: 1},
		{args: commandArgs(graphDigest(graph), "impact-fixture-001", []string{"service/\xff.go"}), input: graph, code: 1},
	}
	for index, test := range tests {
		var stdout, stderr bytes.Buffer
		code := Command(test.args, bytes.NewReader(test.input), &stdout, &stderr)
		if code != test.code || stdout.Len() != 0 || strings.Contains(stderr.String(), secret) {
			t.Errorf("case %d = %d/%q/%q", index, code, stdout.String(), stderr.String())
		}
	}
}

func TestCommandLeavesStdoutUntouchedOnInvalidInput(t *testing.T) {
	graph := marshalGraph(richObservation())
	overPaths := make([]string, maxChangedPaths+1)
	for index := range overPaths {
		overPaths[index] = "service/absent/path" + leftPad(index, 3) + ".go"
	}
	tests := []struct {
		args  []string
		input []byte
	}{
		{args: nil},
		{args: commandArgs(graphDigest(graph), "impact-fixture-001", richChangedPaths), input: []byte("not-json")},
		{args: commandArgs(graphDigest(graph), "impact-fixture-001", []string{"service/z/z.go", "service/d/d.go"}), input: graph},
		{args: commandArgs(graphDigest(graph), "impact-fixture-001", overPaths), input: graph},
		{args: commandArgs(graphDigest(graph), strings.Repeat("r", 161), richChangedPaths), input: graph},
		{args: commandArgs(graphDigest(graph), "impact-fixture-001", []string{strings.Repeat("a", 4_097)}), input: graph},
		{args: commandArgs(strings.Repeat("0", 64), "impact-fixture-001", richChangedPaths), input: bytes.Repeat([]byte{'x'}, maxGraphBytes+1)},
	}
	for _, test := range tests {
		stdout := &countingWriter{}
		var stderr bytes.Buffer
		if code := Command(test.args, bytes.NewReader(test.input), stdout, &stderr); code == 0 {
			t.Fatalf("Command(%v) unexpectedly succeeded", test.args)
		}
		if stdout.calls != 0 || stdout.bytes != 0 {
			t.Fatalf("stdout writes = %d/%d", stdout.calls, stdout.bytes)
		}
	}
}

type countingWriter struct {
	bytes int
	calls int
}

func (writer *countingWriter) Write(value []byte) (int, error) {
	writer.calls++
	writer.bytes += len(value)
	return len(value), nil
}

func commandArgs(hash, runID string, paths []string) []string {
	result := []string{"--graph-sha256", hash, "--run-id", runID}
	for _, path := range paths {
		result = append(result, "--changed-path", path)
	}
	return result
}
