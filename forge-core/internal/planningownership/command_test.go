package planningownership

import (
	"bytes"
	"io"
	"os"
	"path/filepath"
	"testing"
)

func TestCommandEmitsExactGoldenProjection(t *testing.T) {
	fixture := loadGolden(t)
	catalog := readFixture(t, catalogFixturePath)
	mapping := readFixture(t, mappingFixturePath)
	directory := t.TempDir()
	catalogPath := writeTestInput(t, directory, "catalog.yml", catalog)
	mappingPath := writeTestInput(t, directory, "mapping.yml", mapping)
	cases := []struct {
		args  []string
		stdin []byte
	}{
		{[]string{"project", "--catalog", "-", "--mapping", mappingPath}, catalog},
		{[]string{"project", "--mapping", "-", "--catalog", catalogPath}, mapping},
	}
	for index, testCase := range cases {
		stdout, stderr := &bytes.Buffer{}, &bytes.Buffer{}
		code := Command(testCase.args, bytes.NewReader(testCase.stdin), stdout, stderr)
		expected := append(cloneBytes(fixture.projectionRaw), '\n')
		if code != 0 || stderr.Len() != 0 || !bytes.Equal(stdout.Bytes(), expected) {
			t.Fatalf("case %d = code %d stderr %q output exact=%t", index, code, stderr.String(), bytes.Equal(stdout.Bytes(), expected))
		}
	}
}

func TestCommandUsageFailuresWriteNoStdout(t *testing.T) {
	cases := [][]string{
		{}, {"unknown"}, {"help"}, {"-h"}, {"--help"}, {"project"},
		{"project", "--help"}, {"project", "--catalog=-", "--mapping", "x"},
		{"project", "-catalog", "-", "--mapping", "x"},
		{"project", "--catalog", "--weird", "--mapping", "-"},
		{"project", "--catalog", "", "--mapping", "x"},
		{"project", "--catalog", "-", "--mapping", "-"},
		{"project", "--catalog", "a", "--mapping", "b"},
		{"project", "--catalog", "-", "--catalog", "x"},
		{"project", "--catalog", "-", "--mapping", "x", "extra"},
	}
	for index, args := range cases {
		stdout, stderr := &writeCounter{}, &bytes.Buffer{}
		if code := Command(args, bytes.NewReader(nil), stdout, stderr); code != 2 || stdout.writes != 0 || stderr.Len() == 0 {
			t.Fatalf("case %d = code %d writes %d stderr %q", index, code, stdout.writes, stderr.String())
		}
	}
}

func TestCommandRejectsSymlinkAndDirectoryInputs(t *testing.T) {
	directory := t.TempDir()
	target := writeTestInput(t, directory, "mapping.yml", readFixture(t, mappingFixturePath))
	link := filepath.Join(directory, "mapping-link.yml")
	if err := os.Symlink(target, link); err != nil {
		t.Fatal(err)
	}
	for _, path := range []string{link, directory} {
		stdout, stderr := &writeCounter{}, &bytes.Buffer{}
		args := []string{"project", "--catalog", "-", "--mapping", path}
		code := Command(args, bytes.NewReader(readFixture(t, catalogFixturePath)), stdout, stderr)
		if code != 1 || stdout.writes != 0 || stderr.Len() == 0 {
			t.Fatalf("non-regular input %q = %d/%d/%q", path, code, stdout.writes, stderr.String())
		}
	}
}

func TestCommandRejectsSpecialFileWithoutBlockingOrStdout(t *testing.T) {
	directory := t.TempDir()
	pipe := filepath.Join(directory, "mapping.pipe")
	if err := createSpecialTestFile(pipe); err != nil {
		t.Skipf("special file fixture unavailable: %v", err)
	}
	stdout, stderr := &writeCounter{}, &bytes.Buffer{}
	args := []string{"project", "--catalog", "-", "--mapping", pipe}
	if code := Command(args, bytes.NewReader(readFixture(t, catalogFixturePath)), stdout, stderr); code != 1 ||
		stdout.writes != 0 || stderr.Len() == 0 {
		t.Fatalf("special input = %d/%d/%q", code, stdout.writes, stderr.String())
	}
}

func TestRegularReadRejectsPathReplacementAfterRead(t *testing.T) {
	directory := t.TempDir()
	path := writeTestInput(t, directory, "source.yml", []byte("a: one\n"))
	replacement := writeTestInput(t, directory, "replacement.yml", []byte("a: two\n"))
	afterRead := func() {
		if err := os.Rename(replacement, path); err != nil {
			t.Fatal(err)
		}
	}
	if _, err := readRegularFileChecked(path, 128, afterRead); err == nil {
		t.Fatal("path replacement during read was accepted")
	}
}

func TestCommandSemanticFailuresWriteNoStdout(t *testing.T) {
	directory := t.TempDir()
	invalidPath := writeTestInput(t, directory, "mapping.yml", []byte("bad\n"))
	stdout, stderr := &writeCounter{}, &bytes.Buffer{}
	args := []string{"project", "--catalog", "-", "--mapping", invalidPath}
	if code := Command(args, bytes.NewBufferString("bad\n"), stdout, stderr); code != 1 || stdout.writes != 0 || stderr.Len() == 0 {
		t.Fatalf("semantic failure = code %d writes %d stderr %q", code, stdout.writes, stderr.String())
	}
}

func TestCommandRejectsMalformedSequenceItems(t *testing.T) {
	base := readFixture(t, catalogFixturePath)
	directory := t.TempDir()
	mappingPath := writeTestInput(t, directory, "mapping.yml", readFixture(t, mappingFixturePath))
	for name, item := range map[string]string{
		"colon-without-space": "a:x", "two-spaces": " x", "three-spaces": "  x",
		"unmatched-square-colon": "a]: b", "unmatched-curly-colon": "a}: b",
	} {
		t.Run(name, func(t *testing.T) {
			catalog := bytes.Replace(base, []byte("    entry_criteria: [user_or_runtime_intent_exists]\n"),
				[]byte("    entry_criteria:\n      - "+item+"\n"), 1)
			stdout, stderr := &writeCounter{}, &bytes.Buffer{}
			args := []string{"project", "--catalog", "-", "--mapping", mappingPath}
			if code := Command(args, bytes.NewReader(catalog), stdout, stderr); code != 1 ||
				stdout.writes != 0 || stderr.Len() == 0 {
				t.Fatalf("malformed sequence = %d/%d/%q", code, stdout.writes, stderr.String())
			}
		})
	}
}

func TestCommandAcceptsPlainScalarInternalCollectionBytes(t *testing.T) {
	catalog := bytes.Replace(readFixture(t, catalogFixturePath),
		[]byte("    entry_criteria: [user_or_runtime_intent_exists]\n"),
		[]byte("    entry_criteria:\n      - a[\n      - a]\n      - a{\n      - a}\n      - x[y:z]\n      - a,\n      - a,b\n      - a[,\n      - ...\n"), 1)
	catalog = bytes.Replace(catalog, []byte("    inputs: [work_intent_raw, project_snapshot, constitution, adrs, debt, health, policies]\n"),
		[]byte("    inputs: [a[, a{, a, b]\n"), 1)
	directory := t.TempDir()
	mappingPath := writeTestInput(t, directory, "mapping.yml", readFixture(t, mappingFixturePath))
	stdout, stderr := &bytes.Buffer{}, &bytes.Buffer{}
	args := []string{"project", "--catalog", "-", "--mapping", mappingPath}
	if code := Command(args, bytes.NewReader(catalog), stdout, stderr); code != 0 ||
		stdout.Len() == 0 || stderr.Len() != 0 {
		t.Fatalf("plain internal collection bytes = %d/%d/%q", code, stdout.Len(), stderr.String())
	}
}

func TestCommandAcceptsEllipsisSequenceScalar(t *testing.T) {
	catalog := bytes.Replace(readFixture(t, catalogFixturePath),
		[]byte("    entry_criteria: [user_or_runtime_intent_exists]\n"),
		[]byte("    entry_criteria:\n      - ...\n"), 1)
	directory := t.TempDir()
	mappingPath := writeTestInput(t, directory, "mapping.yml", readFixture(t, mappingFixturePath))
	stdout, stderr := &bytes.Buffer{}, &bytes.Buffer{}
	if code := Command([]string{"project", "--catalog", "-", "--mapping", mappingPath},
		bytes.NewReader(catalog), stdout, stderr); code != 0 || stdout.Len() == 0 || stderr.Len() != 0 {
		t.Fatalf("ellipsis sequence scalar = %d/%d/%q", code, stdout.Len(), stderr.String())
	}
}

func TestCommandBoundedReadAndWriteFailures(t *testing.T) {
	if raw, err := readBounded(bytes.NewReader(make([]byte, 8)), 8); err != nil || len(raw) != 8 {
		t.Fatalf("at-bound read = %d/%v", len(raw), err)
	}
	if _, err := readBounded(bytes.NewReader(make([]byte, 9)), 8); err == nil {
		t.Fatal("N+1 read accepted")
	}
	for _, writer := range []io.Writer{shortCommandWriter{}, failedCommandWriter{}} {
		if code := writeCommandOutput(writer, io.Discard, []byte("{}")); code != 1 {
			t.Fatalf("writer %T returned %d", writer, code)
		}
	}
}

func TestCommandDoesNotReadStdinForInvalidOrSecondStdinRequests(t *testing.T) {
	for _, args := range [][]string{
		{"project", "--catalog", "-", "--mapping", "-"},
		{"project", "--catalog", "-", "--mapping"},
	} {
		stdin := &readCounter{}
		stdout, stderr := &writeCounter{}, &bytes.Buffer{}
		if code := Command(args, stdin, stdout, stderr); code != 2 || stdin.reads != 0 || stdout.writes != 0 {
			t.Fatalf("invalid args read stdin or stdout: code=%d reads=%d writes=%d", code, stdin.reads, stdout.writes)
		}
	}
}

func writeTestInput(t *testing.T, directory, name string, raw []byte) string {
	t.Helper()
	path := filepath.Join(directory, name)
	if err := os.WriteFile(path, raw, 0o600); err != nil {
		t.Fatal(err)
	}
	return path
}

type writeCounter struct{ writes int }

func (counter *writeCounter) Write(raw []byte) (int, error) {
	counter.writes++
	return len(raw), nil
}

type shortCommandWriter struct{}

func (shortCommandWriter) Write(raw []byte) (int, error) { return len(raw) - 1, nil }

type failedCommandWriter struct{}

func (failedCommandWriter) Write([]byte) (int, error) { return 0, io.ErrClosedPipe }

type readCounter struct{ reads int }

func (counter *readCounter) Read([]byte) (int, error) {
	counter.reads++
	return 0, io.EOF
}
