package graphterminal

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestCommandBuildsExactReceiptFromStdinAndFile(t *testing.T) {
	fixture := validTerminalFixture(t)
	assertCommandReceipt(t, []string{"--control", "-"}, fixture.ControlJSON, fixture.ReceiptJSON)
	path := filepath.Join(t.TempDir(), "terminal-control.json")
	if err := os.WriteFile(path, fixture.ControlJSON, 0o600); err != nil {
		t.Fatalf("write terminal control: %v", err)
	}
	assertCommandReceipt(t, []string{"--control", path}, nil, fixture.ReceiptJSON)
}

func assertCommandReceipt(t *testing.T, args []string, input, expected []byte) {
	t.Helper()
	var stdout, stderr bytes.Buffer
	code := Command(args, bytes.NewReader(input), &stdout, &stderr)
	if code != 0 || stderr.Len() != 0 || !bytes.Equal(stdout.Bytes(), expected) {
		t.Fatalf("command code=%d stderr=%q\nactual=%s\nexpected=%s",
			code, stderr.String(), stdout.Bytes(), expected)
	}
	if bytes.HasSuffix(stdout.Bytes(), []byte{'\n'}) {
		t.Fatal("terminal receipt output has trailing LF")
	}
}

func TestCommandProtocolVersionIsExactAndExclusive(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if code := Command([]string{"--protocol-version"}, strings.NewReader("secret"), &stdout, &stderr); code != 0 ||
		stdout.String() != "1" || stderr.Len() != 0 {
		t.Fatalf("protocol version code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
	}
	for _, args := range [][]string{
		{"--protocol-version", "--control", "-"}, {"--protocol-version", "extra"},
		{"--protocol-version", "--protocol-version"},
	} {
		stdout.Reset()
		stderr.Reset()
		if code := Command(args, strings.NewReader("secret"), &stdout, &stderr); code != 2 || stdout.Len() != 0 {
			t.Fatalf("args %v code=%d stdout=%q", args, code, stdout.String())
		}
	}
}

func TestCommandRejectsArgumentDriftWithoutEchoingControl(t *testing.T) {
	for _, args := range [][]string{
		nil, {"--control", "-", "extra"}, {"--control", "-", "--control", "-"}, {"--unknown"},
	} {
		var stdout, stderr bytes.Buffer
		if code := Command(args, strings.NewReader("DO_NOT_ECHO"), &stdout, &stderr); code != 2 || stdout.Len() != 0 {
			t.Fatalf("args %v code=%d stdout=%q", args, code, stdout.String())
		}
		if strings.Contains(stderr.String(), "DO_NOT_ECHO") {
			t.Fatalf("private input echoed: %q", stderr.String())
		}
	}
	var stdout, stderr bytes.Buffer
	if code := Command([]string{"--control", "-"}, strings.NewReader("DO_NOT_ECHO"), &stdout, &stderr); code != 1 ||
		stdout.Len() != 0 || strings.Contains(stderr.String(), "DO_NOT_ECHO") {
		t.Fatalf("invalid control code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
	}
}

func TestCommandHelpIsEffectFree(t *testing.T) {
	var stdout, stderr bytes.Buffer
	code := Command([]string{"--help"}, strings.NewReader("secret"), &stdout, &stderr)
	if code != 0 || stdout.Len() != 0 || !strings.Contains(stderr.String(), "effect-free") ||
		!strings.Contains(stderr.String(), "--protocol-version") {
		t.Fatalf("help code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
	}
}
