package graphscheduledcontract

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
)

func TestCommandBuildsExactCandidateFromStdinAndFile(t *testing.T) {
	source := readSourceFixture(t)
	want := mustMarshal(t, mustCandidate(t))
	args := fixtureCommandArgs(source.Input.ExecutionOptions, "-")
	assertCommand(t, args, source.Input.CanonicalControlSnapshotJSON, want)
	path := filepath.Join(t.TempDir(), "control.json")
	if err := os.WriteFile(path, []byte(source.Input.CanonicalControlSnapshotJSON), 0o600); err != nil {
		t.Fatalf("write control: %v", err)
	}
	assertCommand(t, fixtureCommandArgs(source.Input.ExecutionOptions, path), "", want)
}

func assertCommand(t *testing.T, args []string, input string, want []byte) {
	t.Helper()
	var stdout, stderr bytes.Buffer
	code := Command(args, strings.NewReader(input), &stdout, &stderr)
	if code != 0 || stderr.Len() != 0 || !bytes.Equal(stdout.Bytes(), want) {
		t.Fatalf("command code=%d stderr=%q\nactual=%s\nwant=%s", code, stderr.String(), stdout.Bytes(), want)
	}
	if bytes.HasSuffix(stdout.Bytes(), []byte{'\n'}) {
		t.Fatal("candidate output has trailing LF")
	}
}

func TestCommandProtocolVersionIsExactExclusiveAndInputFree(t *testing.T) {
	var stdout, stderr bytes.Buffer
	code := Command([]string{"--protocol-version"}, forbiddenReader{}, &stdout, &stderr)
	if code != 0 || stdout.String() != "2" || stderr.String() != "" {
		t.Fatalf("protocol version code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
	}
	for _, args := range [][]string{
		{"--protocol-version", "--control", "-"},
		{"--protocol-version", "extra"},
		{"--protocol-version", "--protocol-version"},
	} {
		stdout.Reset()
		stderr.Reset()
		code = Command(args, forbiddenReader{}, &stdout, &stderr)
		wantError := "forge graph-scheduled-node-contract: invalid arguments\n"
		if code != 2 || stdout.String() != "" || stderr.String() != wantError {
			t.Fatalf("args=%v code=%d stdout=%q stderr=%q", args, code, stdout.String(), stderr.String())
		}
	}
}

func TestCommandProtocolVersionReportsExactWriteFailure(t *testing.T) {
	var stderr bytes.Buffer
	code := Command([]string{"--protocol-version"}, forbiddenReader{}, shortWriter{}, &stderr)
	want := "forge graph-scheduled-node-contract: cannot write protocol version\n"
	if code != 1 || stderr.String() != want {
		t.Fatalf("protocol write code=%d stderr=%q", code, stderr.String())
	}
}

type forbiddenReader struct{}

func (forbiddenReader) Read([]byte) (int, error) {
	panic("protocol handshake must not read control input")
}

func TestCommandRejectsArgumentsAndPrivateInputWithoutEcho(t *testing.T) {
	valid := fixtureCommandArgs(readSourceFixture(t).Input.ExecutionOptions, "-")
	cases := [][]string{nil, append(valid, "extra"), append(valid, "--control", "-"),
		append([]string(nil), valid[2:]...), append(valid, "--predecessor-receipt", "-"),
		{"--unknown"}}
	for _, args := range cases {
		var stdout, stderr bytes.Buffer
		code := Command(args, strings.NewReader("DO_NOT_ECHO"), &stdout, &stderr)
		if code != 2 || stdout.Len() != 0 || strings.Contains(stderr.String(), "DO_NOT_ECHO") {
			t.Fatalf("args=%v code=%d stdout=%q stderr=%q", args, code, stdout.String(), stderr.String())
		}
	}
	var stdout, stderr bytes.Buffer
	code := Command(valid, strings.NewReader("DO_NOT_ECHO"), &stdout, &stderr)
	if code != 1 || stdout.Len() != 0 || strings.Contains(stderr.String(), "DO_NOT_ECHO") {
		t.Fatalf("invalid control code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
	}
}

func TestCommandNeverTreatsContentWithoutReceiptAsInitial(t *testing.T) {
	valid := fixtureCommandArgs(readSourceFixture(t).Input.ExecutionOptions, "-")
	backend := mustSchedule(t).Nodes[1].NodeID
	for _, args := range [][]string{
		append(append([]string(nil), valid...), "--predecessor-content", "-"),
		append(append([]string(nil), valid...), "--target-node", backend,
			"--predecessor-content", "-"),
	} {
		var stdout, stderr bytes.Buffer
		code := Command(args, bytes.NewReader(controlCanonicalJSON(t)), &stdout, &stderr)
		if code != 2 || stdout.Len() != 0 {
			t.Fatalf("content without receipt: code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
		}
	}
}

func TestCommandRejectsWrongScheduleAndShortWrite(t *testing.T) {
	source := readSourceFixture(t)
	args := fixtureCommandArgs(source.Input.ExecutionOptions, "-")
	args[3] = strings.Repeat("0", 64)
	var stdout, stderr bytes.Buffer
	code := Command(args, strings.NewReader(source.Input.CanonicalControlSnapshotJSON), &stdout, &stderr)
	if code != 1 || stdout.Len() != 0 {
		t.Fatalf("schedule mismatch code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
	}
	args = fixtureCommandArgs(source.Input.ExecutionOptions, "-")
	stderr.Reset()
	code = Command(args, strings.NewReader(source.Input.CanonicalControlSnapshotJSON), shortWriter{}, &stderr)
	if code != 1 {
		t.Fatalf("short write code=%d stderr=%q", code, stderr.String())
	}
}

func TestCommandHelpStatesInitialPassiveFence(t *testing.T) {
	var stdout, stderr bytes.Buffer
	code := Command([]string{"--help"}, strings.NewReader("secret"), &stdout, &stderr)
	for _, phrase := range []string{
		"initial-node-only", "grants no lifecycle", "successor authority",
		"--predecessor-content FILE|-", "--protocol-version",
	} {
		if code != 0 || stdout.Len() != 0 || !strings.Contains(stderr.String(), phrase) {
			t.Fatalf("help missing %q: code=%d stdout=%q stderr=%q", phrase, code, stdout.String(), stderr.String())
		}
	}
}

func TestReadPredecessorContentEnforcesFileAndStdinBounds(t *testing.T) {
	exact := strings.Repeat("x", MaxPredecessorOutputBytes)
	path := filepath.Join(t.TempDir(), "predecessor.txt")
	if err := os.WriteFile(path, []byte(exact), 0o600); err != nil {
		t.Fatalf("write predecessor content: %v", err)
	}
	for name, source := range map[string]string{"file": path, "stdin": "-"} {
		t.Run(name, func(t *testing.T) {
			content, err := readPredecessorContent(source, strings.NewReader(exact))
			if err != nil || content != exact {
				t.Fatalf("exact bound: bytes=%d err=%v", len(content), err)
			}
		})
	}
	if err := os.WriteFile(path, []byte(exact+"x"), 0o600); err != nil {
		t.Fatalf("write oversized predecessor content: %v", err)
	}
	if _, err := readPredecessorContent(path, strings.NewReader("")); err == nil {
		t.Fatal("oversized predecessor content file was accepted")
	}
}

func fixtureCommandArgs(options fixtureOptions, control string) []string {
	return []string{
		"--control", control, "--schedule-sha256", readScheduleSHA(),
		"--endpoint", options.Endpoint, "--model", options.Model,
		"--max-output-tokens", strconv.FormatUint(options.MaxOutputTokens, 10),
		"--max-model-output-bytes", strconv.FormatUint(options.MaxModelOutputBytes, 10),
		"--max-model-events", strconv.FormatUint(options.MaxModelEvents, 10),
		"--timeout-ms", strconv.FormatUint(options.TimeoutMilliseconds, 10),
		"--max-cost-usd-micros", strconv.FormatUint(options.MaxCostUSDMicros, 10),
		"--pricing-snapshot-sha256", options.PricingSnapshotSHA256,
		"--max-result-bytes", strconv.FormatUint(options.MaxResultBytes, 10),
	}
}

func readScheduleSHA() string {
	return "809d5235e4298ea8a66cb0654b0e662b94a8568e4c184cf1a927bda1c46e8148"
}

type shortWriter struct{}

func (shortWriter) Write(data []byte) (int, error) {
	if len(data) == 0 {
		return 0, errors.New("empty write")
	}
	return len(data) - 1, nil
}
