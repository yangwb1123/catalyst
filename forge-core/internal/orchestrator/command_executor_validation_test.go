package orchestrator

import (
	"context"
	"errors"
	"fmt"
	"os/exec"
	"reflect"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
)

func validationTestCommand(t *testing.T, output string) []string {
	t.Helper()
	printf, err := exec.LookPath("printf")
	if err != nil {
		t.Skip("printf not available")
	}
	return []string{printf, output}
}

func TestCommandExecutorPublishesOnlyAfterValidatedCommit(t *testing.T) {
	var calls []string
	ex := CommandExecutor{
		Build: func(asset.Phase, string) []string {
			return validationTestCommand(t, "  exact raw  \n")
		},
		ValidateRawOutput: func(_, output string) error {
			calls = append(calls, "raw:"+output)
			return nil
		},
		ValidateOutput: func(_, output string) error {
			calls = append(calls, "visible:"+output)
			return nil
		},
		CommitValidatedOutput: func(_, raw, output string, _ time.Duration) error {
			calls = append(calls, "commit:"+raw+"|"+output)
			return nil
		},
		Observe: func(_, output string, _ time.Duration) {
			calls = append(calls, "observe:"+output)
		},
	}
	if err := ex.Execute(context.Background(), asset.Phase{Name: "reviewer"}, "balanced"); err != nil {
		t.Fatalf("Execute: %v", err)
	}
	want := []string{
		"raw:  exact raw  \n",
		"visible:exact raw",
		"commit:  exact raw  \n|exact raw",
		"observe:  exact raw  \n",
	}
	if !reflect.DeepEqual(calls, want) {
		t.Fatalf("call order/bytes = %#v, want %#v", calls, want)
	}
}

func TestCommandExecutorRejectedOutputIsNeverPublished(t *testing.T) {
	tests := []struct {
		name      string
		rawErr    error
		outputErr error
		commitErr error
		wantCalls []string
	}{
		{name: "raw", rawErr: errors.New("bad raw"), wantCalls: []string{"raw"}},
		{name: "semantic", outputErr: errors.New("bad semantic"), wantCalls: []string{"raw", "semantic"}},
		{name: "commit", commitErr: errors.New("disk full"), wantCalls: []string{"raw", "semantic", "commit"}},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			var calls []string
			ex := CommandExecutor{
				Build: func(asset.Phase, string) []string { return validationTestCommand(t, "APPROVE") },
				ValidateRawOutput: func(_, _ string) error {
					calls = append(calls, "raw")
					return tc.rawErr
				},
				ValidateOutput: func(_, _ string) error {
					calls = append(calls, "semantic")
					return tc.outputErr
				},
				CommitValidatedOutput: func(_, _, _ string, _ time.Duration) error {
					calls = append(calls, "commit")
					return tc.commitErr
				},
				Observe: func(_, _ string, _ time.Duration) { calls = append(calls, "observe") },
			}
			if err := ex.Execute(context.Background(), asset.Phase{Name: "reviewer"}, "balanced"); err == nil {
				t.Fatal("rejected output must fail")
			}
			if !reflect.DeepEqual(calls, tc.wantCalls) {
				t.Fatalf("calls = %#v, want %#v", calls, tc.wantCalls)
			}
		})
	}
}

func TestCommandExecutorFailedProcessIsNeverPublished(t *testing.T) {
	falsePath, err := exec.LookPath("false")
	if err != nil {
		t.Skip("false not available")
	}
	observed := false
	ex := CommandExecutor{
		Build:   func(asset.Phase, string) []string { return []string{falsePath} },
		Observe: func(_, _ string, _ time.Duration) { observed = true },
	}
	if err := ex.Execute(context.Background(), asset.Phase{Name: "reviewer"}, "balanced"); err == nil {
		t.Fatal("nonzero process must fail")
	}
	if observed {
		t.Fatal("failed process output must not enter accepted Observe sink")
	}
}

func TestCommandExecutorFinalizesBeforeSpawn(t *testing.T) {
	var finalizedArgv []string
	ex := CommandExecutor{
		Build: func(asset.Phase, string) []string { return validationTestCommand(t, "base") },
		FinalizeCommand: func(_ asset.Phase, _ string, argv []string) ([]string, error) {
			argv[len(argv)-1] += "-bound"
			finalizedArgv = append([]string(nil), argv...)
			return argv, nil
		},
		ValidateRawOutput: func(_, output string) error {
			if output != "base-bound" {
				t.Fatalf("spawned output = %q, want finalized argv", output)
			}
			return nil
		},
	}
	if err := ex.Execute(context.Background(), asset.Phase{Name: "reviewer"}, "balanced"); err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if len(finalizedArgv) == 0 {
		t.Fatal("FinalizeCommand was not called")
	}
}

func TestCommandExecutorFinalizeFailurePreventsSpawn(t *testing.T) {
	validated := false
	ex := CommandExecutor{
		Build: func(asset.Phase, string) []string { return validationTestCommand(t, "must-not-run") },
		FinalizeCommand: func(asset.Phase, string, []string) ([]string, error) {
			return nil, errors.New("entropy unavailable")
		},
		ValidateRawOutput: func(_, _ string) error { validated = true; return nil },
	}
	err := ex.Execute(context.Background(), asset.Phase{Name: "reviewer"}, "balanced")
	var execErr *ExecError
	if !errors.As(err, &execErr) || execErr.Kind != KindConfig {
		t.Fatalf("finalize failure = %T %v, want KindConfig", err, err)
	}
	if validated {
		t.Fatal("finalize failure reached spawned-output validation")
	}
}

func TestCommandExecutorCommitsExactExtractedSemanticPayload(t *testing.T) {
	var committed string
	ex := CommandExecutor{
		Build: func(asset.Phase, string) []string { return validationTestCommand(t, "raw-envelope") },
		SemanticOutput: func(raw string) (string, error) {
			if raw != "raw-envelope" {
				t.Fatalf("semantic extractor raw = %q", raw)
			}
			return " exact semantic\n", nil
		},
		ValidateOutput: func(_, semantic string) error {
			if semantic != " exact semantic\n" {
				t.Fatalf("validator semantic = %q", semantic)
			}
			return nil
		},
		CommitValidatedOutput: func(_, _, semantic string, _ time.Duration) error {
			committed = semantic
			return nil
		},
	}
	if err := ex.Execute(context.Background(), asset.Phase{Name: "planner"}, "balanced"); err != nil {
		t.Fatal(err)
	}
	if committed != " exact semantic\n" {
		t.Fatalf("committed semantic = %q", committed)
	}
}

func TestCommandExecutorSemanticExtractionFailureSuppressesCommitAndObserve(t *testing.T) {
	committed, observed := false, false
	ex := CommandExecutor{
		Build:                 func(asset.Phase, string) []string { return validationTestCommand(t, "bad-envelope") },
		SemanticOutput:        func(string) (string, error) { return "", errors.New("bad transport") },
		CommitValidatedOutput: func(_, _, _ string, _ time.Duration) error { committed = true; return nil },
		Observe:               func(_, _ string, _ time.Duration) { observed = true },
	}
	if err := ex.Execute(context.Background(), asset.Phase{Name: "planner"}, "balanced"); err == nil {
		t.Fatal("semantic extraction failure must fail")
	}
	if committed || observed {
		t.Fatalf("extraction failure published state: commit=%v observe=%v", committed, observed)
	}
}

func TestCommandExecutorTruncationStopsEveryMachineOutputHook(t *testing.T) {
	const payload = "0123456789"
	tests := []struct {
		name string
		make func() CommandExecutor
	}{
		{name: "host", make: func() CommandExecutor {
			return CommandExecutor{
				Build:          func(asset.Phase, string) []string { return validationTestCommand(t, payload) },
				MaxOutputBytes: 4,
			}
		}},
		{name: "sandbox", make: func() CommandExecutor {
			executor := sandboxedExecutor(&fakeRunner{output: payload})
			executor.MaxOutputBytes = 4
			return executor
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			calls := []string{}
			executor := test.make()
			executor.ValidateRawOutput = func(string, string) error { calls = append(calls, "raw"); return nil }
			executor.SemanticOutput = func(string) (string, error) { calls = append(calls, "extract"); return "", nil }
			executor.ValidateOutput = func(string, string) error { calls = append(calls, "semantic"); return nil }
			executor.CommitValidatedOutput = func(string, string, string, time.Duration) error {
				calls = append(calls, "commit")
				return nil
			}
			executor.Observe = func(string, string, time.Duration) { calls = append(calls, "observe") }

			err := executor.Execute(context.Background(), asset.Phase{Name: "reviewer"}, "balanced")
			assertOutputTruncation(t, err, 4, int64(len(payload)))
			if len(calls) != 0 {
				t.Fatalf("truncated output crossed machine hooks: %v", calls)
			}
		})
	}
}

func assertOutputTruncation(t *testing.T, err error, retained int, total int64) {
	t.Helper()
	var execErr *ExecError
	if !errors.As(err, &execErr) || execErr.Kind != KindFailed || execErr.Phase != "reviewer" {
		t.Fatalf("truncation error = %T %v, want terminal ExecError", err, err)
	}
	want := fmt.Sprintf("retained %d of %d child bytes", retained, total)
	if !strings.Contains(err.Error(), want) {
		t.Fatalf("truncation facts missing from %v, want %q", err, want)
	}
}
