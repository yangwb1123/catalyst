package graphscheduledreconcile

import (
	"bytes"
	"errors"
	"strings"
	"testing"
)

func TestCommandProtocolVersionIsExactAndExclusive(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if code := Command([]string{"--protocol-version"}, strings.NewReader("private"), &stdout, &stderr); code != 0 ||
		stdout.String() != "1" || stderr.Len() != 0 {
		t.Fatalf("protocol command = code %d stdout %q stderr %q", code, stdout.String(), stderr.String())
	}
	for _, args := range [][]string{
		{"--protocol-version", "--snapshot", "-"}, {"--protocol-version", "extra"},
		{"--protocol-version", "--protocol-version"},
	} {
		stdout.Reset()
		stderr.Reset()
		if code := Command(args, strings.NewReader("private"), &stdout, &stderr); code != 2 || stdout.Len() != 0 {
			t.Fatalf("Command(%v) = code %d stdout %q", args, code, stdout.String())
		}
	}
}

func TestCommandReconcilesStdinWithoutTrailingNewline(t *testing.T) {
	input := signedSnapshotBytes(t, validUnsignedSnapshot())
	var stdout, stderr bytes.Buffer
	code := Command([]string{"--snapshot", "-"}, bytes.NewReader(input), &stdout, &stderr)
	if code != 0 || stderr.Len() != 0 || stdout.Len() == 0 || stdout.Bytes()[stdout.Len()-1] == '\n' {
		t.Fatalf("command = code %d stdout %q stderr %q", code, stdout.String(), stderr.String())
	}
	if !strings.Contains(stdout.String(), `"disposition":"ready"`) {
		t.Fatalf("decision = %s", stdout.Bytes())
	}
}

func TestCommandFailsClosedWithoutLeakingInput(t *testing.T) {
	secret := `{"v":1,"private":"do-not-print"}`
	var stdout, stderr bytes.Buffer
	if code := Command([]string{"--snapshot", "-"}, strings.NewReader(secret), &stdout, &stderr); code != 1 {
		t.Fatalf("Command code = %d", code)
	}
	if stdout.Len() != 0 || strings.Contains(stderr.String(), "do-not-print") ||
		stderr.String() != "forge graph-scheduled-reconcile: invalid progress snapshot\n" {
		t.Fatalf("stdout %q stderr %q", stdout.String(), stderr.String())
	}
}

func TestCommandHelpAndArgumentValidation(t *testing.T) {
	for _, args := range [][]string{nil, {"--snapshot", "-", "extra"}, {"--snapshot", "-", "--snapshot", "-"}} {
		var stdout, stderr bytes.Buffer
		if code := Command(args, strings.NewReader(""), &stdout, &stderr); code != 2 || stdout.Len() != 0 {
			t.Fatalf("Command(%v) = code %d stdout %q", args, code, stdout.String())
		}
	}
	var stdout, stderr bytes.Buffer
	if code := Command([]string{"--help"}, strings.NewReader(""), &stdout, &stderr); code != 0 ||
		stdout.Len() != 0 || !strings.Contains(stderr.String(), "--protocol-version") {
		t.Fatalf("help = code %d stdout %q stderr %q", code, stdout.String(), stderr.String())
	}
}

type failingWriter struct{}

func (failingWriter) Write([]byte) (int, error) { return 0, errors.New("write failed") }

func TestCommandRejectsOutputFailure(t *testing.T) {
	input := signedSnapshotBytes(t, validUnsignedSnapshot())
	var stderr bytes.Buffer
	if code := Command([]string{"--snapshot", "-"}, bytes.NewReader(input), failingWriter{}, &stderr); code != 1 ||
		!strings.Contains(stderr.String(), "cannot write reconcile decision") {
		t.Fatalf("Command = code %d stderr %q", code, stderr.String())
	}
}
