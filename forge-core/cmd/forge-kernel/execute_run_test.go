package main

import (
	"bytes"
	"errors"
	"io"
	"reflect"
	"testing"

	"forgeos/forge-core/internal/bootstrapgrantissuance"
	"forgeos/forge-core/internal/bootstrapreporeadexecution"
)

func TestRunExecutionPassesClosedFlagsAndClearsReturnedBuffer(t *testing.T) {
	want := executionConfigFixture()
	var got bootstrapreporeadexecution.Config
	output := []byte(`{"delivery_disposition":"first_delivery"}`)
	var stdout, stderr bytes.Buffer
	execute := func(config bootstrapreporeadexecution.Config) ([]byte, error) {
		got = config
		return output, nil
	}
	code := runKernel(validExecutionArgs(), &stdout, &stderr, nil, execute)
	if code != 0 || stderr.Len() != 0 || !reflect.DeepEqual(got, want) {
		t.Fatalf("exit/stderr/config = %d/%q/%#v", code, stderr.String(), got)
	}
	if stdout.String() != `{"delivery_disposition":"first_delivery"}` {
		t.Fatalf("stdout = %q", stdout.String())
	}
	if !bytes.Equal(output, make([]byte, len(output))) {
		t.Fatal("execution delivery buffer was not cleared")
	}
}

func TestRunExecutionUsesStableErrorAndUncertainDeliveryCodes(t *testing.T) {
	t.Run("operation", func(t *testing.T) {
		var stdout, stderr bytes.Buffer
		execute := func(bootstrapreporeadexecution.Config) ([]byte, error) {
			return nil, &bootstrapreporeadexecution.Error{
				Code: bootstrapreporeadexecution.CodeManifestRejected,
				Err:  errors.New("private content"),
			}
		}
		code := runKernel(validExecutionArgs(), &stdout, &stderr, nil, execute)
		if code != 1 || stdout.Len() != 0 || stderr.String() != "forge-kernel: MANIFEST_REJECTED\n" {
			t.Fatalf("exit/stdout/stderr = %d/%q/%q", code, stdout.String(), stderr.String())
		}
	})
	t.Run("delivery", func(t *testing.T) {
		var stderr bytes.Buffer
		execute := func(bootstrapreporeadexecution.Config) ([]byte, error) {
			return []byte(`{"result":"raw"}`), nil
		}
		code := runKernel(validExecutionArgs(), &controlledWriter{count: 1},
			&stderr, nil, execute)
		if code != 1 || stderr.String() != "forge-kernel: OUTPUT_DELIVERY_UNCERTAIN\n" {
			t.Fatalf("exit/stderr = %d/%q", code, stderr.String())
		}
	})
}

func TestRunExecutionRejectsMissingExtraAndAmbientArguments(t *testing.T) {
	for key, value := range map[string]string{
		"FORGE_REPO_ROOT": "/ambient/repo", "FORGE_EXECUTION_ROOT": "/ambient/root",
	} {
		t.Setenv(key, value)
	}
	invalid := [][]string{
		{"execute-bootstrap-repo-read"},
		append(validExecutionArgs(), "--now=1"),
		append(validExecutionArgs(), "--force"),
		append(validExecutionArgs(), "positional"),
	}
	for index, args := range invalid {
		var stdout, stderr bytes.Buffer
		called := false
		execute := func(bootstrapreporeadexecution.Config) ([]byte, error) {
			called = true
			return []byte("{}"), nil
		}
		code := runKernel(args, &stdout, &stderr, nil, execute)
		if code != 2 || called || stdout.Len() != 0 ||
			stderr.String() != "forge-kernel: INVALID_ARGUMENTS\n" {
			t.Errorf("case %d exit/called/stdout/stderr = %d/%v/%q/%q",
				index, code, called, stdout.String(), stderr.String())
		}
	}
}

func TestRunKernelKeepsIssuanceAndExecutionDispatchSeparate(t *testing.T) {
	issueCalled, executeCalled := false, false
	issue := func(bootstrapgrantissuance.Config) ([]byte, error) {
		issueCalled = true
		return []byte("{}"), nil
	}
	execute := func(bootstrapreporeadexecution.Config) ([]byte, error) {
		executeCalled = true
		return []byte("{}"), nil
	}
	if code := runKernel(validArgs(), io.Discard, io.Discard, issue, execute); code != 0 {
		t.Fatalf("issuance exit = %d", code)
	}
	if !issueCalled || executeCalled {
		t.Fatalf("issuance dispatch = %v/%v", issueCalled, executeCalled)
	}
}

func TestRunExecutionOutputFailureCanOnlyReplayContentFreeTerminal(t *testing.T) {
	committed := false
	execute := func(bootstrapreporeadexecution.Config) ([]byte, error) {
		if committed {
			return []byte(`{"delivery_disposition":"exact_replay","execution_result":null}`), nil
		}
		committed = true
		return []byte(`{"delivery_disposition":"first_delivery","execution_result":{"raw":true}}`), nil
	}
	var firstError bytes.Buffer
	code := runKernel(validExecutionArgs(), &controlledWriter{err: errors.New("broken pipe")},
		&firstError, nil, execute)
	if code != 1 || !committed ||
		firstError.String() != "forge-kernel: OUTPUT_DELIVERY_UNCERTAIN\n" {
		t.Fatalf("first exit/committed/stderr = %d/%v/%q", code, committed, firstError.String())
	}
	var replay, replayError bytes.Buffer
	code = runKernel(validExecutionArgs(), &replay, &replayError, nil, execute)
	if code != 0 || replayError.Len() != 0 ||
		replay.String() != `{"delivery_disposition":"exact_replay","execution_result":null}` {
		t.Fatalf("replay exit/stdout/stderr = %d/%q/%q", code, replay.String(), replayError.String())
	}
}

func executionConfigFixture() bootstrapreporeadexecution.Config {
	return bootstrapreporeadexecution.Config{
		RepositoryRoot: "/repository", AuthorityRoot: "/authority", StateDir: "state",
		IssuanceTrustRootPath: "issuance-root.json", IssuanceLedgerPath: "issuance-ledger.json",
		ExecutionTrustRootPath: "execution-root.json", ExecutionPolicyPath: "execution-policy.json",
		InvocationPath: "invocation.json", ManifestPath: "manifest.json",
		ReceiptSeedPath:           "execution-receipt.seed",
		PinnedIssuanceRootSHA256:  string(bytes.Repeat([]byte{'a'}, 64)),
		PinnedExecutionRootSHA256: string(bytes.Repeat([]byte{'b'}, 64)),
	}
}

func validExecutionArgs() []string {
	config := executionConfigFixture()
	return []string{"execute-bootstrap-repo-read", "--repository-root=" + config.RepositoryRoot,
		"--authority-root=" + config.AuthorityRoot, "--state-dir=" + config.StateDir,
		"--issuance-trust-root-path=" + config.IssuanceTrustRootPath,
		"--issuance-ledger-path=" + config.IssuanceLedgerPath,
		"--execution-trust-root-path=" + config.ExecutionTrustRootPath,
		"--execution-policy-path=" + config.ExecutionPolicyPath,
		"--invocation-path=" + config.InvocationPath, "--manifest-path=" + config.ManifestPath,
		"--receipt-seed-path=" + config.ReceiptSeedPath,
		"--pinned-issuance-root-sha256=" + config.PinnedIssuanceRootSHA256,
		"--pinned-execution-root-sha256=" + config.PinnedExecutionRootSHA256}
}
