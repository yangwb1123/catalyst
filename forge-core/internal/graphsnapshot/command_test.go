package graphsnapshot

import (
	"bytes"
	"io"
	"os"
	"path/filepath"
	"testing"
)

func TestCommandEmitsExactProjectionFromStdin(t *testing.T) {
	graph, digest, observation := loadFixtureGraph(t)
	want, err := Build(graph, digest, observation.Producer.RunID, "fixture-catalyst-go")
	if err != nil {
		t.Fatal(err)
	}
	stdout, stderr := &bytes.Buffer{}, &bytes.Buffer{}
	code := Command(commandArgs(digest, observation.Producer.RunID, "fixture-catalyst-go", "-"),
		bytes.NewReader(graph), stdout, stderr)
	if code != 0 || stderr.Len() != 0 || !bytes.Equal(stdout.Bytes(), want.JSON()) {
		t.Fatalf("command = %d, stderr %q, output match %v", code, stderr.String(), bytes.Equal(stdout.Bytes(), want.JSON()))
	}
}

func TestCommandExplicitProfilesAreExactAndLegacyRemainsDefault(t *testing.T) {
	graph, digest, observation := loadFixtureGraph(t)
	legacy, err := Build(graph, digest, observation.Producer.RunID, "fixture-catalyst-go")
	if err != nil {
		t.Fatal(err)
	}
	testSource, err := BuildTestSource(
		graph, digest, observation.Producer.RunID, "fixture-catalyst-go")
	if err != nil {
		t.Fatal(err)
	}
	base := commandArgs(digest, observation.Producer.RunID, "fixture-catalyst-go", "-")
	assertCommandOutput(t, base, graph, legacy.JSON())
	assertCommandOutput(t, append(base, "--profile", profileID), graph, legacy.JSON())
	assertCommandOutput(t, append(base, "--profile", testSourceProfileID), graph, testSource.JSON())
}

func assertCommandOutput(t *testing.T, args []string, graph, want []byte) {
	t.Helper()
	stdout, stderr := &bytes.Buffer{}, &bytes.Buffer{}
	code := Command(args, bytes.NewReader(graph), stdout, stderr)
	if code != 0 || stderr.Len() != 0 || !bytes.Equal(stdout.Bytes(), want) {
		t.Fatalf("command = %d stderr %q exact=%v", code, stderr.String(),
			bytes.Equal(stdout.Bytes(), want))
	}
}

func TestCommandReadsOnlyTheExplicitFile(t *testing.T) {
	graph, digest, observation := loadFixtureGraph(t)
	path := filepath.Join(t.TempDir(), "graph.json")
	if err := os.WriteFile(path, graph, 0o600); err != nil {
		t.Fatal(err)
	}
	stdout, stderr := &bytes.Buffer{}, &bytes.Buffer{}
	code := Command(commandArgs(digest, observation.Producer.RunID, "fixture-catalyst-go", path),
		bytes.NewBufferString("poison stdin"), stdout, stderr)
	if code != 0 || stdout.Len() == 0 || stderr.Len() != 0 {
		t.Fatalf("explicit file command = %d, stdout %d, stderr %q", code, stdout.Len(), stderr.String())
	}
}

func TestCommandFailureLeavesStdoutUntouched(t *testing.T) {
	graph, _, observation := loadFixtureGraph(t)
	stdout, stderr := &countingWriter{}, &bytes.Buffer{}
	code := Command(commandArgs("bad", observation.Producer.RunID, "fixture-catalyst-go", "-"),
		bytes.NewReader(graph), stdout, stderr)
	if code == 0 || stdout.writes != 0 || stderr.Len() == 0 {
		t.Fatalf("rejection = %d, writes %d, stderr %q", code, stdout.writes, stderr.String())
	}
}

func TestCommandArgumentAndInputFailuresNeverWriteStdout(t *testing.T) {
	graph, digest, observation := loadFixtureGraph(t)
	cases := []struct {
		args []string
		code int
		raw  []byte
	}{
		{args: []string{}, code: 2, raw: graph},
		{args: append(commandArgs(digest, observation.Producer.RunID, "fixture", "-"), "trailing"), code: 2, raw: graph},
		{args: append(commandArgs(digest, observation.Producer.RunID, "fixture", "-"), "--run-id", observation.Producer.RunID), code: 2, raw: graph},
		{args: commandArgs("bad", observation.Producer.RunID, "fixture", "-"), code: 1, raw: graph},
		{args: commandArgs(digest, observation.Producer.RunID, "fixture", "-"), code: 1, raw: []byte("bad")},
		{args: commandArgs(digest, observation.Producer.RunID, "fixture", "-"), code: 1, raw: make([]byte, maxGraphBytes+1)},
	}
	for index, testCase := range cases {
		stdout, stderr := &countingWriter{}, &bytes.Buffer{}
		code := Command(testCase.args, bytes.NewReader(testCase.raw), stdout, stderr)
		if code != testCase.code || stdout.writes != 0 || stderr.Len() == 0 {
			t.Fatalf("case %d = code %d writes %d stderr %q", index, code, stdout.writes, stderr.String())
		}
	}
}

func TestCommandRejectsInvalidProfileArgumentsBeforeOutput(t *testing.T) {
	graph, digest, observation := loadFixtureGraph(t)
	base := commandArgs(digest, observation.Producer.RunID, "fixture", "-")
	cases := [][]string{
		append(append([]string{}, base...), "--profile", ""),
		append(append([]string{}, base...), "--profile", "future-profile/v2"),
		append(append([]string{}, base...), "--profile", profileID, "--profile", testSourceProfileID),
		append(append([]string{}, base...), "--profile", testSourceProfileID, "positional"),
	}
	for index, args := range cases {
		stdout, stderr := &countingWriter{}, &bytes.Buffer{}
		if code := Command(args, bytes.NewReader(graph), stdout, stderr); code != 2 ||
			stdout.writes != 0 || stderr.Len() == 0 {
			t.Fatalf("case %d = code %d writes %d stderr %q", index, code,
				stdout.writes, stderr.String())
		}
	}
}

func TestCommandMapsShortOrFailedStdoutWriteToFailure(t *testing.T) {
	graph, digest, observation := loadFixtureGraph(t)
	for _, writer := range []io.Writer{shortWriter{}, errorWriter{}} {
		code := Command(commandArgs(digest, observation.Producer.RunID, "fixture", "-"),
			bytes.NewReader(graph), writer, io.Discard)
		if code != 1 {
			t.Fatalf("stdout writer %T returned code %d", writer, code)
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

func commandArgs(digest, runID, projectID, input string) []string {
	return []string{
		"--project-id", projectID, "--graph-sha256", digest,
		"--run-id", runID, "--input", input,
	}
}
