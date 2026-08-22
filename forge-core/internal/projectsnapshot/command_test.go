package projectsnapshot

import (
	"bytes"
	"testing"
)

type shortCommandWriter struct{}

func (shortCommandWriter) Write(value []byte) (int, error) {
	return len(value) - 1, nil
}

func TestCommandSuccessWritesCanonicalJSONPlusOneLF(t *testing.T) {
	root, environment := snapshotFixture(t)
	var stdout, stderr bytes.Buffer
	code := commandWith([]string{
		"capture", "--run-id", "run-1", "--root", root, "--project-id", "project-1",
	}, environment, &stdout, &stderr)
	if code != 0 || stderr.Len() != 0 || stdout.Len() < 2 ||
		stdout.Bytes()[stdout.Len()-1] != '\n' || stdout.Bytes()[stdout.Len()-2] == '\n' {
		t.Fatalf("command code=%d stdout=%q stderr=%q", code, stdout.Bytes(), stderr.Bytes())
	}
}

func TestCommandInvalidArgumentsUseExitTwoAndNoStdout(t *testing.T) {
	tests := [][]string{
		nil,
		{"capture", "--project-id", "p", "--run-id", "r"},
		{"wrong", "--project-id", "p", "--run-id", "r", "--root", "."},
		{"capture", "--project-id", "p", "--project-id", "q", "--root", "."},
		{"capture", "--project-id", "p", "--run-id", "r", "--unknown", "."},
		{"capture", "--project-id", "p", "--run-id", "r", "--root", "."},
		{"capture", "--project-id", "p", "--run-id", "r", "--root", "/tmp/../tmp"},
	}
	for _, args := range tests {
		var stdout, stderr bytes.Buffer
		if code := commandWith(args, []string{"PATH=/usr/bin"}, &stdout, &stderr); code != 2 || stdout.Len() != 0 || stderr.String() != commandUsage {
			t.Errorf("args=%v code=%d stdout=%q stderr=%q", args, code, stdout.Bytes(), stderr.Bytes())
		}
	}
}

func TestCommandCaptureFailureWritesNoStdout(t *testing.T) {
	var stdout, stderr bytes.Buffer
	code := commandWith([]string{
		"capture", "--project-id", "p", "--run-id", "r", "--root", "/no/such/repo",
	}, []string{"PATH=/usr/bin"}, &stdout, &stderr)
	if code != 1 || stdout.Len() != 0 || stderr.Len() == 0 {
		t.Fatalf("capture failure code=%d stdout=%q stderr=%q", code, stdout.Bytes(), stderr.Bytes())
	}
}

func TestCommandShortWriteFailsClosed(t *testing.T) {
	root, environment := snapshotFixture(t)
	var stderr bytes.Buffer
	code := commandWith([]string{
		"capture", "--run-id", "run-1", "--root", root, "--project-id", "project-1",
	}, environment, shortCommandWriter{}, &stderr)
	if code != 1 || stderr.Len() == 0 {
		t.Fatalf("short write code=%d stderr=%q", code, stderr.Bytes())
	}
}
