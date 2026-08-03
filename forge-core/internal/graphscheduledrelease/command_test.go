package graphscheduledrelease

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestCommandAuthorizesExactStdinAndFile(t *testing.T) {
	control, encoded := validReleaseFixture(t)
	wantValue, err := BuildAuthorization(control)
	if err != nil {
		t.Fatalf("BuildAuthorization: %v", err)
	}
	want, err := MarshalAuthorization(wantValue)
	if err != nil {
		t.Fatalf("MarshalAuthorization: %v", err)
	}
	assertCommandOutput(t, []string{"--control", "-"}, encoded, want)
	path := filepath.Join(t.TempDir(), "release-control.json")
	if err := os.WriteFile(path, encoded, 0o600); err != nil {
		t.Fatalf("write control: %v", err)
	}
	assertCommandOutput(t, []string{"--control", path}, nil, want)
}

func assertCommandOutput(t *testing.T, args []string, input, want []byte) {
	t.Helper()
	var stdout, stderr bytes.Buffer
	if code := Command(args, bytes.NewReader(input), &stdout, &stderr); code != 0 {
		t.Fatalf("Command code=%d stderr=%q", code, stderr.String())
	}
	if !bytes.Equal(stdout.Bytes(), want) || stderr.Len() != 0 {
		t.Fatalf("Command output=%q stderr=%q", stdout.Bytes(), stderr.String())
	}
}

func TestCommandRejectsDriftWithoutDisclosingPrivateInput(t *testing.T) {
	control, encoded := validReleaseFixture(t)
	private := control.ScheduledContract.Request.SystemPrompt
	mutated := append(append([]byte(nil), encoded...), '\n')
	var stdout, stderr bytes.Buffer
	if code := Command([]string{"--control", "-"}, bytes.NewReader(mutated), &stdout, &stderr); code != 1 {
		t.Fatalf("Command code=%d stderr=%q", code, stderr.String())
	}
	if stdout.Len() != 0 || strings.Contains(stderr.String(), private) ||
		strings.Contains(stderr.String(), control.ProviderRequestJSON) {
		t.Fatalf("failure disclosed private input: stdout=%q stderr=%q", stdout.String(), stderr.String())
	}
}

func TestCommandHelpArgumentsAndShortWrite(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if code := Command([]string{"--help"}, strings.NewReader("private"), &stdout, &stderr); code != 0 ||
		!strings.Contains(stderr.String(), "performs no external effect") {
		t.Fatalf("help code=%d stderr=%q", code, stderr.String())
	}
	stderr.Reset()
	if code := Command([]string{}, strings.NewReader("private"), &stdout, &stderr); code != 2 {
		t.Fatalf("missing args code=%d", code)
	}
	_, encoded := validReleaseFixture(t)
	stderr.Reset()
	if code := Command([]string{"--control", "-"}, bytes.NewReader(encoded), shortWriter{}, &stderr); code != 1 {
		t.Fatalf("short write code=%d stderr=%q", code, stderr.String())
	}
}

type shortWriter struct{}

func (shortWriter) Write(value []byte) (int, error) {
	if len(value) == 0 {
		return 0, nil
	}
	return len(value) - 1, nil
}
