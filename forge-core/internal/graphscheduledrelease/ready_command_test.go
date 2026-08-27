package graphscheduledrelease

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestReadyCommandProtocolVersionIsExactAndExclusive(t *testing.T) {
	var stdout, stderr bytes.Buffer
	code := ReadyCommand(
		[]string{"--protocol-version"}, strings.NewReader("private"), &stdout, &stderr,
	)
	if code != 0 || stdout.String() != "2" || stderr.Len() != 0 {
		t.Fatalf("protocol code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
	}
	for _, args := range [][]string{
		{"--protocol-version", "--control", "-"}, {"--control", "-", "--protocol-version"},
	} {
		stdout.Reset()
		stderr.Reset()
		if code := ReadyCommand(args, strings.NewReader("private"), &stdout, &stderr); code != 2 ||
			stdout.Len() != 0 {
			t.Fatalf("nonexclusive protocol args=%v code=%d stdout=%q", args, code, stdout.String())
		}
	}
}

func TestReadyCommandAuthorizesExactStdinAndFile(t *testing.T) {
	control, encoded := validReadyInitialFixture(t)
	wantValue, err := BuildReadyAuthorization(control)
	if err != nil {
		t.Fatalf("BuildReadyAuthorization: %v", err)
	}
	want, err := MarshalReadyAuthorization(wantValue)
	if err != nil {
		t.Fatalf("MarshalReadyAuthorization: %v", err)
	}
	assertReadyCommandOutputTest(t, []string{"--control", "-"}, encoded, want)
	path := filepath.Join(t.TempDir(), "ready-release-control.json")
	if err := os.WriteFile(path, encoded, 0o600); err != nil {
		t.Fatalf("write ready control: %v", err)
	}
	assertReadyCommandOutputTest(t, []string{"--control", path}, nil, want)
}

func assertReadyCommandOutputTest(t *testing.T, args []string, input, want []byte) {
	t.Helper()
	var stdout, stderr bytes.Buffer
	if code := ReadyCommand(args, bytes.NewReader(input), &stdout, &stderr); code != 0 {
		t.Fatalf("ReadyCommand code=%d stderr=%q", code, stderr.String())
	}
	if !bytes.Equal(stdout.Bytes(), want) || stderr.Len() != 0 {
		t.Fatalf("ReadyCommand output=%q stderr=%q", stdout.Bytes(), stderr.String())
	}
}

func TestReadyCommandRejectsPrivateDriftAndShortWrite(t *testing.T) {
	control, encoded := validReadyInitialFixture(t)
	private := control.ScheduledContract.Request.SystemPrompt
	mutated := append(append([]byte(nil), encoded...), '\n')
	var stdout, stderr bytes.Buffer
	code := ReadyCommand([]string{"--control", "-"}, bytes.NewReader(mutated), &stdout, &stderr)
	if code != 1 || stdout.Len() != 0 || strings.Contains(stderr.String(), private) ||
		strings.Contains(stderr.String(), control.ProviderRequestJSON) {
		t.Fatalf("failure leaked private input: code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
	}
	stderr.Reset()
	code = ReadyCommand([]string{"--control", "-"}, bytes.NewReader(encoded), shortWriter{}, &stderr)
	if code != 1 || !strings.Contains(stderr.String(), "cannot write") {
		t.Fatalf("short write code=%d stderr=%q", code, stderr.String())
	}
}
