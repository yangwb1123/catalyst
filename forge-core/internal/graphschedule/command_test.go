package graphschedule

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestCommandBuildsExactScheduleFromStdinAndFile(t *testing.T) {
	control := readExistingControlFixture(t).Input.CanonicalControlSnapshotJSON
	want := readScheduleFixture(t).CanonicalExecutionScheduleJSON
	assertScheduleCommand(t, []string{"--control", "-"}, control, want)
	path := filepath.Join(t.TempDir(), "control.json")
	if err := os.WriteFile(path, []byte(control), 0o600); err != nil {
		t.Fatalf("write control: %v", err)
	}
	assertScheduleCommand(t, []string{"--control", path}, "", want)
}

func assertScheduleCommand(t *testing.T, args []string, input, want string) {
	t.Helper()
	var stdout, stderr bytes.Buffer
	code := Command(args, strings.NewReader(input), &stdout, &stderr)
	if code != 0 || stderr.Len() != 0 || stdout.String() != want {
		t.Fatalf("command code=%d stderr=%q\nactual=%s\nwant=%s",
			code, stderr.String(), stdout.String(), want)
	}
	if bytes.HasSuffix(stdout.Bytes(), []byte{'\n'}) {
		t.Fatal("schedule output has trailing LF")
	}
}

func TestCommandRejectsDriftWithoutEchoingPrivateControl(t *testing.T) {
	for _, args := range [][]string{
		nil, {"--control", "-", "extra"}, {"--control", "-", "--control", "-"}, {"--unknown"},
	} {
		var stdout, stderr bytes.Buffer
		code := Command(args, strings.NewReader("DO_NOT_ECHO"), &stdout, &stderr)
		if code != 2 || stdout.Len() != 0 || strings.Contains(stderr.String(), "DO_NOT_ECHO") {
			t.Fatalf("args=%v code=%d stdout=%q stderr=%q", args, code, stdout.String(), stderr.String())
		}
	}
	var stdout, stderr bytes.Buffer
	code := Command([]string{"--control", "-"}, strings.NewReader("DO_NOT_ECHO"), &stdout, &stderr)
	if code != 1 || stdout.Len() != 0 || strings.Contains(stderr.String(), "DO_NOT_ECHO") {
		t.Fatalf("invalid control code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
	}
}

func TestCommandHelpIsPassiveAndEffectFree(t *testing.T) {
	var stdout, stderr bytes.Buffer
	code := Command([]string{"--help"}, strings.NewReader("secret"), &stdout, &stderr)
	if code != 0 || stdout.Len() != 0 || !strings.Contains(stderr.String(), "passive static schedule") ||
		!strings.Contains(stderr.String(), "grants no execution") {
		t.Fatalf("help code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
	}
}

func TestCommandRejectsShortWrites(t *testing.T) {
	control := readExistingControlFixture(t).Input.CanonicalControlSnapshotJSON
	var stderr bytes.Buffer
	if code := Command([]string{"--control", "-"}, strings.NewReader(control), shortWriter{}, &stderr); code != 1 {
		t.Fatalf("short write code=%d stderr=%q", code, stderr.String())
	}
}

type shortWriter struct{}

func (shortWriter) Write(data []byte) (int, error) {
	if len(data) == 0 {
		return 0, errors.New("empty write")
	}
	return len(data) - 1, nil
}
