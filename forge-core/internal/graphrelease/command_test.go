package graphrelease

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestCommandAuthorizesExactStdinWithoutTrailingLF(t *testing.T) {
	control, input := validReleaseFixture(t)
	expectedValue, err := BuildAuthorization(control)
	if err != nil {
		t.Fatalf("build expected authorization: %v", err)
	}
	expected, err := MarshalAuthorization(expectedValue)
	if err != nil {
		t.Fatalf("marshal expected authorization: %v", err)
	}
	var stdout, stderr bytes.Buffer
	code := Command([]string{"--control", "-"}, bytes.NewReader(input), &stdout, &stderr)
	if code != 0 || stderr.Len() != 0 || !bytes.Equal(stdout.Bytes(), expected) {
		t.Fatalf("Command code=%d stderr=%q\nactual=%s\nexpected=%s",
			code, stderr.String(), stdout.Bytes(), expected)
	}
	if bytes.HasSuffix(stdout.Bytes(), []byte{'\n'}) {
		t.Fatal("command output has a trailing LF")
	}
}

func TestCommandReadsExactFileAndRejectsArgumentDrift(t *testing.T) {
	_, input := validReleaseFixture(t)
	path := filepath.Join(t.TempDir(), "release-control.json")
	if err := os.WriteFile(path, input, 0o600); err != nil {
		t.Fatalf("write control: %v", err)
	}
	var stdout, stderr bytes.Buffer
	if code := Command([]string{"--control", path}, strings.NewReader(""), &stdout, &stderr); code != 0 || stdout.Len() == 0 || stderr.Len() != 0 {
		t.Fatalf("file command code=%d stdout=%d stderr=%q", code, stdout.Len(), stderr.String())
	}
	for _, args := range [][]string{
		nil,
		{"--control", "-", "extra"},
		{"--control", "-", "--control", "-"},
		{"--unknown", "value"},
	} {
		stdout.Reset()
		stderr.Reset()
		if code := Command(args, bytes.NewReader(input), &stdout, &stderr); code != 2 {
			t.Fatalf("args %v returned %d, want 2", args, code)
		}
		if stdout.Len() != 0 {
			t.Fatalf("args %v wrote stdout", args)
		}
	}
}

func TestCommandFailureNeverEchoesPrivateInput(t *testing.T) {
	private := `{"private_prompt":"DO_NOT_ECHO"}`
	var stdout, stderr bytes.Buffer
	code := Command(
		[]string{"--control", "-"}, strings.NewReader(private), &stdout, &stderr,
	)
	if code != 1 || stdout.Len() != 0 {
		t.Fatalf("invalid input code=%d stdout=%q", code, stdout.String())
	}
	if strings.Contains(stderr.String(), "DO_NOT_ECHO") ||
		!strings.Contains(stderr.String(), "invalid release control") {
		t.Fatalf("unsafe or unclear stderr: %q", stderr.String())
	}
}

func TestCommandHelpIsEffectFree(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if code := Command([]string{"--help"}, strings.NewReader("secret"), &stdout, &stderr); code != 0 || stdout.Len() != 0 ||
		!strings.Contains(stderr.String(), "graph-node-dispatch-authorize") ||
		!strings.Contains(stderr.String(), "private authorization artifact") ||
		!strings.Contains(stderr.String(), "does not release dispatch authority") {
		t.Fatalf("help code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
	}
}
