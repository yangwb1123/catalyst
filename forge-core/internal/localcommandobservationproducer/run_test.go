package localcommandobservationproducer

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	commandcontract "forgeos/forge-core/internal/commandobservationevidencecontract"
	"forgeos/forge-core/internal/execbound"
)

func TestRunProducesRawFullAndRetainedStreamIdentities(t *testing.T) {
	root := runFixture(t, "printf abcdef; printf XYZ >&2; exit 7")
	result := Run(context.Background(), root, CommandGate, "run-streams", execbound.Options{
		MaxOutputBytes: 3,
	}, execbound.CaptureCombined)
	if result.ObservationError != nil || result.Production == nil {
		t.Fatalf("production failed: %+v", result)
	}
	observation := result.Production.Package().Observation
	if observation.Termination.Kind != "exited" || observation.Termination.ExitCode == nil ||
		*observation.Termination.ExitCode != 7 {
		t.Fatalf("nonzero process observation lost: %#v", observation.Termination)
	}
	assertWireStream(t, observation.Streams.Stdout, "abcdef", "abc")
	assertWireStream(t, observation.Streams.Stderr, "XYZ", "XYZ")
	if observation.Streams.Combined.Bytes != 9 || observation.Streams.Combined.RetainedBytes != 3 {
		t.Fatalf("combined stream counts = %#v", observation.Streams.Combined)
	}
	if result.Execution.Legacy.Total != 9 || result.Execution.Legacy.Retained != 3 {
		t.Fatalf("legacy projection counts = %#v", result.Execution.Legacy)
	}
}

func TestRunUsesTheExactScrubbedEnvironmentForTheActualChild(t *testing.T) {
	t.Setenv("OPENAI_API_KEY", "must-not-reach-child")
	t.Setenv("HTTPS_PROXY", "must-not-reach-child")
	t.Setenv("SAFE_CAPTURE_VALUE", "visible")
	root := runFixture(t, `
if [ "${OPENAI_API_KEY+x}" = x ] || [ "${HTTPS_PROXY+x}" = x ]; then
  printf leaked-secret
  exit 42
fi
printf '%s' "$SAFE_CAPTURE_VALUE"
`)
	result := Run(context.Background(), root, CommandGate, "run-environment", execbound.Options{}, execbound.CaptureCombined)
	if result.ObservationError != nil || result.Production == nil ||
		result.Execution.Legacy.Observed() != "visible" {
		t.Fatalf("actual child environment did not match the scrubbed profile: %+v", result)
	}
	for _, variable := range result.Production.Package().EnvironmentManifest.Variables {
		if variable.Name == "OPENAI_API_KEY" || variable.Name == "HTTPS_PROXY" {
			t.Fatalf("secret/proxy variable reached manifest: %#v", variable)
		}
	}
}

func TestRunTimeoutProducesStandaloneObservation(t *testing.T) {
	root := runFixture(t, "exec /bin/sleep 30")
	result := Run(context.Background(), root, CommandGate, "run-timeout",
		execbound.Options{Timeout: 100 * time.Millisecond}, execbound.CaptureCombined)
	if result.ObservationError != nil || result.Production == nil {
		t.Fatalf("timeout must remain a standalone observation: %+v", result)
	}
	termination := result.Production.Package().Observation.Termination
	if termination.Kind != "timed_out" || termination.ExitCode != nil {
		t.Fatalf("timeout termination = %#v", termination)
	}
}

func TestRunCancellationProducesObservationWithinBoundedPostcheck(t *testing.T) {
	startedMarker := filepath.Join(t.TempDir(), "started")
	t.Setenv("RUN_STARTED_MARKER", startedMarker)
	root := runFixture(t, `: > "$RUN_STARTED_MARKER"; exec /bin/sleep 30`)
	ctx, cancel := context.WithCancel(context.Background())
	done := make(chan RunResult, 1)
	go func() {
		done <- Run(ctx, root, CommandGate, "run-cancelled",
			execbound.Options{Unbounded: true}, execbound.CaptureCombined)
	}()
	waitForFixtureMarker(t, startedMarker)
	cancel()
	select {
	case result := <-done:
		if result.ObservationError != nil || result.Production == nil ||
			result.Production.Package().Observation.Termination.Kind != "cancelled" {
			t.Fatalf("cancelled execution was not sealed honestly: %+v", result)
		}
	case <-time.After(postCancellationProfileGrace + 5*time.Second):
		t.Fatal("cancelled producer did not return within its postcheck bound")
	}
}

func TestRunPreflightAndPostflightHonorCallerCancellation(t *testing.T) {
	t.Run("preflight", func(t *testing.T) {
		root := t.TempDir()
		fixture := cancelFixtureTools(t, root, true)
		t.Setenv("PATH", fixture.bin)
		ctx, cancel := context.WithCancel(context.Background())
		done := make(chan RunResult, 1)
		go func() {
			done <- Run(ctx, root, CommandGate, "run-preflight-cancel",
				execbound.Options{Unbounded: true}, execbound.CaptureCombined)
		}()
		waitForFixtureMarkerWhileRunning(t, fixture.gitMarker, done)
		started := time.Now()
		cancel()
		result := <-done
		if time.Since(started) > 3*time.Second || result.ObservationError == nil ||
			result.Execution.Execution.Started {
			t.Fatalf("preflight cancellation was not prompt/fail-closed: %+v", result)
		}
		if _, err := os.Stat(fixture.targetMarker); !os.IsNotExist(err) {
			t.Fatalf("target command spawned during canceled preflight: %v", err)
		}
	})

	t.Run("postflight", func(t *testing.T) {
		root := t.TempDir()
		fixture := cancelFixtureTools(t, root, false)
		t.Setenv("PATH", fixture.bin)
		ctx, cancel := context.WithCancel(context.Background())
		done := make(chan RunResult, 1)
		go func() {
			done <- Run(ctx, root, CommandGate, "run-postflight-cancel",
				execbound.Options{Unbounded: true}, execbound.CaptureCombined)
		}()
		waitForFixtureMarkerWhileRunning(t, fixture.postMarker, done)
		started := time.Now()
		cancel()
		result := <-done
		if time.Since(started) > 3*time.Second || result.ObservationError == nil ||
			result.Production != nil || result.Execution.Legacy.Err != nil ||
			result.Execution.Legacy.Observed() != "target-ok" {
			t.Fatalf("postflight cancellation changed command result or hung: %+v", result)
		}
	})
}

func TestPostExecutionProfileContextOnlyDetachesForControllerTermination(t *testing.T) {
	parent, cancelParent := context.WithCancel(context.Background())
	cancelParent()

	for _, termination := range []execbound.TerminationKind{
		execbound.TerminationExited,
		execbound.TerminationSignaled,
		execbound.TerminationNotStarted,
	} {
		ctx, cancel := postExecutionProfileContext(parent, termination)
		if !errors.Is(ctx.Err(), context.Canceled) {
			cancel()
			t.Fatalf("termination %q detached from caller cancellation", termination)
		}
		cancel()
	}

	for _, termination := range []execbound.TerminationKind{
		execbound.TerminationCancelled,
		execbound.TerminationTimedOut,
	} {
		ctx, cancel := postExecutionProfileContext(parent, termination)
		if ctx.Err() != nil {
			cancel()
			t.Fatalf("termination %q did not receive bounded sealing grace: %v", termination, ctx.Err())
		}
		deadline, ok := ctx.Deadline()
		if !ok || time.Until(deadline) <= 0 || time.Until(deadline) > postCancellationProfileGrace {
			cancel()
			t.Fatalf("termination %q has invalid sealing deadline: %v, %t", termination, deadline, ok)
		}
		cancel()
	}

	healthyParent := context.Background()
	exitedContext, cancelExited := postExecutionProfileContext(healthyParent, execbound.TerminationExited)
	if _, hasDeadline := exitedContext.Deadline(); hasDeadline {
		cancelExited()
		t.Fatal("natural exit received an unrelated post-execution deadline")
	}
	cancelExited()
	timedOutContext, cancelTimedOut := postExecutionProfileContext(healthyParent, execbound.TerminationTimedOut)
	if deadline, hasDeadline := timedOutContext.Deadline(); !hasDeadline ||
		time.Until(deadline) <= 0 || time.Until(deadline) > postCancellationProfileGrace {
		cancelTimedOut()
		t.Fatal("timed-out execution did not receive bounded post-execution verification")
	}
	cancelTimedOut()
}

func TestRunRejectsUnproducibleInputsBeforeSpawn(t *testing.T) {
	for _, test := range []struct {
		name    string
		runID   string
		capture execbound.Capture
		opts    execbound.Options
	}{
		{name: "invalid run id", runID: "INVALID", capture: execbound.CaptureCombined},
		{name: "wrong capture", runID: "run-valid", capture: execbound.CaptureSplit},
		{name: "fractional milliseconds", runID: "run-valid", capture: execbound.CaptureCombined,
			opts: execbound.Options{Timeout: time.Millisecond + time.Nanosecond}},
	} {
		t.Run(test.name, func(t *testing.T) {
			result := Run(context.Background(), t.TempDir(), CommandGate, test.runID, test.opts, test.capture)
			if result.ObservationError == nil || result.Production != nil ||
				result.Execution.Execution.Started ||
				result.Execution.Execution.Termination != execbound.TerminationNotStarted {
				t.Fatalf("invalid preflight was not fail-closed: %+v", result)
			}
		})
	}
}

func TestRunSignalAndSourceDriftProduceNoPackage(t *testing.T) {
	for _, test := range []struct {
		name string
		body string
	}{
		{name: "signal", body: "kill -TERM $$"},
		{name: "source drift", body: "printf changed > tracked.txt; exit 0"},
	} {
		t.Run(test.name, func(t *testing.T) {
			root := runFixture(t, test.body)
			result := Run(context.Background(), root, CommandGate, "run-"+strings.ReplaceAll(test.name, " ", "-"),
				execbound.Options{}, execbound.CaptureCombined)
			if result.ObservationError == nil || result.Production != nil {
				t.Fatalf("unproducible execution returned package: %+v", result)
			}
			if !result.Execution.Execution.Started {
				t.Fatalf("command must still have executed: %+v", result.Execution.Execution)
			}
		})
	}
}

func runFixture(t *testing.T, body string) string {
	t.Helper()
	sanitizeFixtureEnvironment(t)
	root, _ := sourceFixture(t)
	bin := t.TempDir()
	node := filepath.Join(bin, "node")
	if err := os.WriteFile(node, []byte("#!/bin/sh\n"+body+"\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	gitPath, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", strings.Join([]string{bin, filepath.Dir(gitPath)}, string(filepath.ListSeparator)))
	return root
}

func assertWireStream(t *testing.T, stream commandcontract.Stream, full, retained string) {
	t.Helper()
	fullDigest, retainedDigest := sha256.Sum256([]byte(full)), sha256.Sum256([]byte(retained))
	if stream.Bytes != int64(len(full)) || stream.RetainedBytes != int64(len(retained)) ||
		stream.SHA256 != hex.EncodeToString(fullDigest[:]) ||
		stream.RetainedSHA256 != hex.EncodeToString(retainedDigest[:]) {
		t.Fatalf("stream identity = %#v", stream)
	}
}

func waitForFixtureMarker(t *testing.T, path string) {
	t.Helper()
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		if _, err := os.Stat(path); err == nil {
			return
		}
		time.Sleep(10 * time.Millisecond)
	}
	t.Fatalf("fixture marker %q was not created", path)
}

func waitForFixtureMarkerWhileRunning(t *testing.T, path string, done <-chan RunResult) {
	t.Helper()
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		if _, err := os.Stat(path); err == nil {
			return
		}
		select {
		case result := <-done:
			t.Fatalf("producer returned before fixture marker %q: %+v", path, result)
		default:
		}
		time.Sleep(10 * time.Millisecond)
	}
	t.Fatalf("fixture marker %q was not created", path)
}

type cancellationFixture struct {
	bin          string
	gitMarker    string
	postMarker   string
	targetMarker string
}

func cancelFixtureTools(t *testing.T, root string, stallPreflight bool) cancellationFixture {
	t.Helper()
	sanitizeFixtureEnvironment(t)
	bin := t.TempDir()
	targetMarker := filepath.Join(t.TempDir(), "target")
	gitMarker := filepath.Join(t.TempDir(), "git")
	postMarker := filepath.Join(t.TempDir(), "post")
	counter := filepath.Join(t.TempDir(), "counter")
	nodeBody := "#!/bin/sh\n: > " + shellSingleQuote(targetMarker) + "\nprintf target-ok\n"
	if err := os.WriteFile(filepath.Join(bin, "node"), []byte(nodeBody), 0o755); err != nil {
		t.Fatal(err)
	}
	gitBody := fakeGitBody(stallPreflight, root, gitMarker, postMarker, counter)
	if err := os.WriteFile(filepath.Join(bin, "git"), []byte(gitBody), 0o755); err != nil {
		t.Fatal(err)
	}
	return cancellationFixture{
		bin:          bin,
		gitMarker:    gitMarker,
		postMarker:   postMarker,
		targetMarker: targetMarker,
	}
}

func fakeGitBody(stallPreflight bool, root, gitMarker, postMarker, counter string) string {
	if stallPreflight {
		return "#!/bin/sh\n: > " + shellSingleQuote(gitMarker) + "\nexec /bin/sleep 30\n"
	}
	return `#!/bin/sh
count=0
counter=` + shellSingleQuote(counter) + `
post_marker=` + shellSingleQuote(postMarker) + `
root=` + shellSingleQuote(root) + `
if [ -f "$counter" ]; then read count < "$counter"; fi
count=$((count + 1))
printf '%s' "$count" > "$counter"
if [ "$count" -gt 5 ]; then
  : > "$post_marker"
  exec /bin/sleep 30
fi
case "$*" in
  *"rev-parse --show-toplevel"*) printf '%s\n' "$root" ;;
  *"rev-parse --show-object-format"*) printf 'sha1\n' ;;
  *"rev-parse --verify HEAD"*) printf '0000000000000000000000000000000000000001\n' ;;
  *"ls-files --cached --stage -z"*) : ;;
  *"ls-files --others --exclude-standard -z"*) : ;;
  *) exit 2 ;;
esac
`
}

func shellSingleQuote(value string) string {
	return "'" + strings.ReplaceAll(value, "'", "'\"'\"'") + "'"
}
