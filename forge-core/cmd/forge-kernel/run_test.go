package main

import (
	"bytes"
	"errors"
	"io"
	"reflect"
	"testing"

	"forgeos/forge-core/internal/bootstrapgrantissuance"
)

func TestRunWritesExactCanonicalBytesOnlyAfterSuccess(t *testing.T) {
	for _, disposition := range []string{"stored", "exact_replay"} {
		t.Run(disposition, func(t *testing.T) {
			var stdout, stderr bytes.Buffer
			expected := []byte(`{"delivery_disposition":"` + disposition + `"}`)
			issue := func(config bootstrapgrantissuance.Config) ([]byte, error) {
				if stdout.Len() != 0 {
					t.Fatal("stdout was written before issuance returned")
				}
				return expected, nil
			}
			if code := run(validArgs(), &stdout, &stderr, issue); code != 0 {
				t.Fatalf("exit = %d, stderr=%q", code, stderr.String())
			}
			if !bytes.Equal(stdout.Bytes(), expected) || stderr.Len() != 0 {
				t.Fatalf("stdout/stderr = %q / %q", stdout.Bytes(), stderr.Bytes())
			}
			if bytes.HasSuffix(stdout.Bytes(), []byte("\n")) {
				t.Fatal("canonical stdout has a trailing LF")
			}
		})
	}
}

func TestRunFailureWritesNoStdoutAndOnlyStableCode(t *testing.T) {
	var stdout, stderr bytes.Buffer
	secret := errors.New("private seed bytes must not be printed")
	issue := func(bootstrapgrantissuance.Config) ([]byte, error) {
		return nil, &bootstrapgrantissuance.Error{
			Code: bootstrapgrantissuance.CodeIssuerKeyRejected, Err: secret,
		}
	}
	if code := run(validArgs(), &stdout, &stderr, issue); code != 1 {
		t.Fatalf("exit = %d", code)
	}
	if stdout.Len() != 0 || stderr.String() != "forge-kernel: ISSUER_KEY_REJECTED\n" {
		t.Fatalf("stdout/stderr = %q / %q", stdout.String(), stderr.String())
	}
}

func TestRunDoesNotPrintUnrecognizedErrorCode(t *testing.T) {
	var stdout, stderr bytes.Buffer
	issue := func(bootstrapgrantissuance.Config) ([]byte, error) {
		return nil, &bootstrapgrantissuance.Error{Code: "BAD\nsecret", Err: errors.New("secret")}
	}
	if code := run(validArgs(), &stdout, &stderr, issue); code != 1 {
		t.Fatalf("exit = %d", code)
	}
	if stdout.Len() != 0 || stderr.String() != "forge-kernel: INTERNAL_ERROR\n" {
		t.Fatalf("stdout/stderr = %q / %q", stdout.String(), stderr.String())
	}
}

func TestRunRejectsEmptySuccessOutput(t *testing.T) {
	var stdout, stderr bytes.Buffer
	issue := func(bootstrapgrantissuance.Config) ([]byte, error) { return []byte{}, nil }
	if code := run(validArgs(), &stdout, &stderr, issue); code != 1 {
		t.Fatalf("exit = %d", code)
	}
	if stdout.Len() != 0 || stderr.String() != "forge-kernel: INTERNAL_ERROR\n" {
		t.Fatalf("stdout/stderr = %q / %q", stdout.String(), stderr.String())
	}
}

func TestRunFailsClosedOnStdoutShortWriteOrError(t *testing.T) {
	for _, writer := range []*controlledWriter{
		{count: 1},
		{err: errors.New("output unavailable")},
	} {
		var stderr bytes.Buffer
		issue := func(bootstrapgrantissuance.Config) ([]byte, error) {
			return []byte("{}"), nil
		}
		if code := run(validArgs(), writer, &stderr, issue); code != 1 {
			t.Fatalf("exit = %d", code)
		}
		if stderr.String() != "forge-kernel: OUTPUT_FAILED\n" {
			t.Fatalf("stderr = %q", stderr.String())
		}
	}
}

func TestRunCanRecoverCommittedDecisionThroughExactReplayAfterOutputFailure(t *testing.T) {
	committed := false
	issue := func(bootstrapgrantissuance.Config) ([]byte, error) {
		if committed {
			return []byte(`{"delivery_disposition":"exact_replay"}`), nil
		}
		committed = true
		return []byte(`{"delivery_disposition":"stored"}`), nil
	}
	var firstError bytes.Buffer
	if code := run(validArgs(), &controlledWriter{err: errors.New("broken pipe")},
		&firstError, issue); code != 1 || !committed {
		t.Fatalf("first delivery = %d, committed=%v", code, committed)
	}
	var replay, replayError bytes.Buffer
	if code := run(validArgs(), &replay, &replayError, issue); code != 0 {
		t.Fatalf("replay delivery = %d, stderr=%q", code, replayError.String())
	}
	if replay.String() != `{"delivery_disposition":"exact_replay"}` {
		t.Fatalf("replay output = %q", replay.String())
	}
}

func TestRunAcceptsOnlyIssueBootstrapAndClosedFlags(t *testing.T) {
	invalid := [][]string{
		nil, {"keygen"}, {"execute"}, {"issue-bootstrap"},
		append(validArgs(), "--now=1"), append(validArgs(), "--force"),
		append(validArgs(), "--execute"), append(validArgs(), "positional"),
	}
	for index, args := range invalid {
		var stdout, stderr bytes.Buffer
		called := false
		issue := func(bootstrapgrantissuance.Config) ([]byte, error) {
			called = true
			return []byte("{}"), nil
		}
		if code := run(args, &stdout, &stderr, issue); code != 2 || called {
			t.Errorf("case %d exit/called = %d/%v", index, code, called)
		}
		if stdout.Len() != 0 || stderr.String() != "forge-kernel: INVALID_ARGUMENTS\n" {
			t.Errorf("case %d stdout/stderr = %q/%q", index, stdout.String(), stderr.String())
		}
	}
}

func TestRunDoesNotUseEnvironmentFallback(t *testing.T) {
	for key, value := range map[string]string{
		"FORGE_REPO_ROOT": "/env/repo", "FORGE_AUTHORITY_ROOT": "/env/authority",
		"FORGE_ROOT_SHA256": string(bytes.Repeat([]byte{'a'}, 64)),
	} {
		t.Setenv(key, value)
	}
	var stdout, stderr bytes.Buffer
	called := false
	code := run([]string{"issue-bootstrap"}, &stdout, &stderr,
		func(bootstrapgrantissuance.Config) ([]byte, error) { called = true; return nil, nil })
	if code != 2 || called || stdout.Len() != 0 {
		t.Fatalf("environment fallback exit/called/stdout = %d/%v/%q", code, called, stdout.String())
	}
}

func TestRunPassesEveryExplicitFlagExactly(t *testing.T) {
	want := bootstrapgrantissuance.Config{
		RepositoryRoot: "/repository", AuthorityRoot: "/authority", StateDir: "state",
		TrustRootPath: "root.json", PolicyPath: "policy.json", RequestPath: "request.json",
		IssuerSeedPath: "issuer.seed", PinnedRootSHA256: string(bytes.Repeat([]byte{'a'}, 64)),
	}
	var got bootstrapgrantissuance.Config
	code := run(validArgs(), io.Discard, io.Discard,
		func(config bootstrapgrantissuance.Config) ([]byte, error) { got = config; return []byte("{}"), nil })
	if code != 0 || !reflect.DeepEqual(got, want) {
		t.Fatalf("exit/config = %d/%#v", code, got)
	}
}

func validArgs() []string {
	return []string{"issue-bootstrap", "--repository-root=/repository",
		"--authority-root=/authority", "--state-dir=state", "--trust-root-path=root.json",
		"--policy-path=policy.json", "--request-path=request.json",
		"--issuer-seed-path=issuer.seed",
		"--pinned-root-sha256=" + string(bytes.Repeat([]byte{'a'}, 64))}
}

type controlledWriter struct {
	count int
	err   error
}

func (writer *controlledWriter) Write(value []byte) (int, error) {
	if writer.err != nil {
		return 0, writer.err
	}
	if writer.count < len(value) {
		return writer.count, nil
	}
	return len(value), nil
}
