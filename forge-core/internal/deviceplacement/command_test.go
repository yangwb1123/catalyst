package deviceplacement

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestCommandReadsStdinAndEmitsExpectedCanonicalResult(t *testing.T) {
	request := validRequest()
	input := marshalRequest(t, request)
	expected, err := Evaluate(request)
	if err != nil {
		t.Fatal(err)
	}
	expectedJSON, err := Marshal(expected)
	if err != nil {
		t.Fatal(err)
	}
	var stdout, stderr bytes.Buffer
	code := Command([]string{"dry-run", "--input", "-"}, bytes.NewReader(input), &stdout, &stderr)
	if code != 0 || stderr.Len() != 0 || !bytes.Equal(stdout.Bytes(), expectedJSON) {
		t.Fatalf("Command = %d, %s, %s", code, stdout.Bytes(), stderr.Bytes())
	}
}

func TestCommandReadsBoundedRegularFile(t *testing.T) {
	path := filepath.Join(t.TempDir(), "request.json")
	if err := os.WriteFile(path, marshalRequest(t, validRequest()), 0o600); err != nil {
		t.Fatal(err)
	}
	var stdout, stderr bytes.Buffer
	if code := Command([]string{"dry-run", "--input", path}, nil, &stdout, &stderr); code != 0 || stdout.Len() == 0 {
		t.Fatalf("file command = %d, stdout=%q stderr=%q", code, stdout.String(), stderr.String())
	}
}

func TestCommandRejectsInvalidInvocationWithoutEchoingInput(t *testing.T) {
	secret := "PRIVATE-DECLARATION-VALUE"
	inputs := [][]string{
		nil,
		{"unknown"},
		{"dry-run"},
		{"dry-run", "--input", "-", "--input", secret},
		{"dry-run", "--input", "-", "--network", secret},
		{"dry-run", "--input", "-", secret},
	}
	for _, args := range inputs {
		var stdout, stderr bytes.Buffer
		code := Command(args, strings.NewReader(secret), &stdout, &stderr)
		if code != 2 || stdout.Len() != 0 || strings.Contains(stderr.String(), secret) {
			t.Errorf("args=%v result=%d stdout=%q stderr=%q", args, code, stdout.String(), stderr.String())
		}
	}
}

func TestCommandRejectsMalformedAndOversizeDocuments(t *testing.T) {
	inputs := [][]byte{[]byte(`{"schema_version":"wrong"}`), bytes.Repeat([]byte{' '}, MaxRequestBytes+1)}
	for _, input := range inputs {
		var stdout, stderr bytes.Buffer
		if code := Command([]string{"dry-run", "--input", "-"}, bytes.NewReader(input), &stdout, &stderr); code != 1 || stdout.Len() != 0 {
			t.Errorf("result=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
		}
	}
}

func TestCommandHelpAndShortWrite(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if code := Command([]string{"dry-run", "--help"}, nil, &stdout, &stderr); code != 0 || stdout.Len() != 0 ||
		!strings.Contains(stderr.String(), "unverified") {
		t.Fatalf("help = %d, %q, %q", code, stdout.String(), stderr.String())
	}
	stderr.Reset()
	if code := Command([]string{"dry-run", "--input", "-"}, bytes.NewReader(marshalRequest(t, validRequest())), shortWriter{}, &stderr); code != 1 ||
		!strings.Contains(stderr.String(), "cannot write") {
		t.Fatalf("short write = %d, %q", code, stderr.String())
	}
}

type shortWriter struct{}

func (shortWriter) Write(data []byte) (int, error) {
	if len(data) == 0 {
		return 0, nil
	}
	return len(data) - 1, nil
}
