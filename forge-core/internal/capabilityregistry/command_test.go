package capabilityregistry

import (
	"bytes"
	"io"
	"testing"
)

func TestCommandArgumentFailuresLeaveStdoutUntouched(t *testing.T) {
	cases := [][]string{
		{}, {"unknown"}, {"help"}, {"-h"}, {"--help"}, {"validate"},
		{"validate", "--help"}, {"validate", "--registry=-"},
		{"validate", "-registry", "-"}, {"validate", "--registry", ""},
		{"help", "extra"}, {"validate", "--help", "extra"},
		{"validate", "--registry", "-", "extra"},
		{"validate", "--registry", "-", "--registry", "-"},
		{"resolve", "--registry", "-"},
		{"resolve", "--registry", "-", "--request", "-"},
		{"resolve", "--registry", "a", "--request", "b"},
	}
	for index, args := range cases {
		stdout, stderr := &countingWriter{}, &bytes.Buffer{}
		if code := Command(args, bytes.NewReader(nil), stdout, stderr); code != 2 ||
			stdout.writes != 0 || stderr.Len() == 0 {
			t.Fatalf("case %d = code %d writes %d stderr %q", index, code,
				stdout.writes, stderr.String())
		}
	}
}

func TestCommandMalformedInputsFailWithoutOutput(t *testing.T) {
	for _, args := range [][]string{
		{"validate", "--registry", "-"},
		{"resolve", "--registry", "-", "--request", "missing.json"},
	} {
		stdout, stderr := &countingWriter{}, &bytes.Buffer{}
		if code := Command(args, bytes.NewBufferString("{}"), stdout, stderr); code != 1 ||
			stdout.writes != 0 || stderr.Len() == 0 {
			t.Fatalf("args %v = code %d writes %d stderr %q", args, code,
				stdout.writes, stderr.String())
		}
	}
}

func TestBoundedReaderAtAndOverLimit(t *testing.T) {
	if raw, err := readBounded(bytes.NewReader(make([]byte, 8)), 8); err != nil || len(raw) != 8 {
		t.Fatalf("within bound = %d/%v", len(raw), err)
	}
	if _, err := readBounded(bytes.NewReader(make([]byte, 9)), 8); err == nil {
		t.Fatal("over bound accepted")
	}
}

func TestCommandMapsShortOrFailedWriteToFailure(t *testing.T) {
	for _, writer := range []io.Writer{shortWriter{}, errorWriter{}} {
		if code := writeCommandOutput(writer, io.Discard, []byte("{}"), "test"); code != 1 {
			t.Fatalf("writer %T returned %d", writer, code)
		}
	}
}

type countingWriter struct{ writes int }

func (value *countingWriter) Write(raw []byte) (int, error) {
	value.writes++
	return len(raw), nil
}

type shortWriter struct{}

func (shortWriter) Write(raw []byte) (int, error) { return len(raw) - 1, nil }

type errorWriter struct{}

func (errorWriter) Write([]byte) (int, error) { return 0, io.ErrClosedPipe }
