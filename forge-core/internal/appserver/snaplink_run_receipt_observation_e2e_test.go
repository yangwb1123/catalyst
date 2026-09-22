package appserver

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/deviceplacement"
)

func runObservationTUI(t *testing.T, executable, apiURL, accessToken, conversationID, runID, sessionObservationInput string) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("the Run observation TUI integration requires a PTY launcher named script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader(fmt.Sprintf("open %s\nruns\nrun-observed %q\nsession-observation-preview --input %s\ntimeline %q\nquit\n", conversationID, runID, sessionObservationInput, runID))
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Run observation TUI failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("Run observation TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"Run \"" + runID + "\" status=\"completed\"",
		"remote Run observed [forge.run.observed.v1]",
		"offline session device observation [forge.session-device-observation/v1]",
		"offline session device observation [forge.session-device-observation/v1] at 200000",
		"owner=" + snaplinkForgeTestUser + " conversation=" + conversationID + " run=" + runID,
		"resources: devices=9 runner_instances=9 cpu=66 memory=135168 storage=67584 gpus=0 gpu_memory=0 eligible_devices=2 eligible_instances=2",
		"candidate-a/runner-a: matches resources=cpu:8 memory:16384 storage:8192 gpu:false gpu_memory:0",
		"candidate-i/runner-i: matches resources=cpu:8 memory:16384 storage:8192 gpu:false gpu_memory:0",
		"authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false reservation_created=false execution_authorized=false dispatch_performed=false",
		"Event seq=1",
		"type=\"run_started\"",
		"type=\"run_finished\"",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("Run observation TUI output omitted %q: %q", want, output)
		}
	}
	if strings.Contains(output, "private event payload") || strings.Contains(output, "tool_name") ||
		strings.Contains(output, "output_path") {
		t.Fatalf("Run observation TUI leaked timeline payload: %q", output)
	}
}

func runSessionRunnerReceiptTUI(
	t *testing.T,
	executable, apiURL, accessToken, conversationID, promptID, runID, receiptInput,
	expectedDisposition string,
	expectedUncertain bool,
) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("the session Runner receipt TUI integration requires a PTY launcher named script: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader(fmt.Sprintf(
		"open %s\nruns\nsession-runner-receipt-preview --input %s\nquit\n",
		conversationID, receiptInput,
	))
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("session Runner receipt TUI failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("session Runner receipt TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	expectedReceiptLine := fmt.Sprintf(
		"receipt_command=command-1 attempt=attempt-1 target=runner-1 disposition=%s",
		expectedDisposition,
	)
	expectedUncertainLine := fmt.Sprintf("receipt_valid=true uncertain=%t", expectedUncertain)
	expectedFollowUp := "follow_up=none reconciliation_required=false manual_review_required=false automatic_retry=false"
	if expectedUncertain {
		expectedFollowUp = "follow_up=reconciliation_manual reconciliation_required=true manual_review_required=true automatic_retry=false"
	}
	for _, want := range []string{
		"authenticated session Runner terminal receipt observation [forge.session-runner-receipt-observation/v1]",
		"owner=" + snaplinkForgeTestUser + " conversation=" + conversationID + " prompt=" + promptID + " run=" + runID,
		expectedReceiptLine,
		expectedUncertainLine,
		expectedFollowUp,
		"selected_target=none",
		"receipt_persisted=false execution_authorized=false",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("session Runner receipt TUI output omitted %q: %q", want, output)
		}
	}
	if strings.Contains(output, "forge-task") || strings.Contains(output, "--prompt-ref") ||
		strings.Contains(output, "private event payload") || strings.Contains(output, "/api/v1/devices") {
		t.Fatalf("session Runner receipt TUI leaked command payload or device request: %q", output)
	}
}

func runSessionRunnerReceiptPreviewE2E(
	t *testing.T,
	executable, apiURL, accessToken, conversationID, promptID, runID string,
	owner deviceplacement.Owner,
	commandID, commandSHA256 string,
	observationJSON []byte,
	recorder *conversationHTTPRecorder,
	previewPath, expectedDisposition string,
	expectedUncertain bool,
) {
	t.Helper()
	inputPath := filepath.Join(t.TempDir(), "session-runner-receipt-observation.json")
	if err := os.WriteFile(inputPath, observationJSON, 0o600); err != nil {
		t.Fatal(err)
	}
	firstCLIRequest := len(recorder.snapshot())
	output, stderr, err := runForgeRuntimeCLI(t, executable, apiURL, accessToken, t.TempDir(),
		"--json", "remote", "session-runner-receipt", "preview", "--input", inputPath)
	if err != nil {
		t.Fatalf("authenticated CLI session Runner receipt preview failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var observation deviceplacement.SessionRunnerReceiptObservation
	if err := json.Unmarshal([]byte(output), &observation); err != nil {
		t.Fatalf("decode authenticated CLI session Runner receipt observation: %v stdout=%q", err, output)
	}
	if err := observation.Validate(); err != nil || observation.Owner != owner ||
		observation.ConversationID != conversationID || observation.PromptID != promptID ||
		observation.RunID != runID || observation.SelectedTargetID != nil ||
		observation.Authority != (deviceplacement.SessionRunnerReceiptAuthority{}) ||
		observation.ReceiptObservation.CommandID != commandID ||
		observation.ReceiptObservation.CommandSHA256 != commandSHA256 ||
		observation.ReceiptObservation.DispositionKind != expectedDisposition ||
		observation.ReceiptObservation.Uncertain != expectedUncertain ||
		observation.ReceiptObservation.ReconciliationRequired != expectedUncertain ||
		observation.ReceiptObservation.ManualReviewRequired != expectedUncertain ||
		observation.ReceiptObservation.AutomaticRetry ||
		observation.ReceiptObservation.FollowUp != map[bool]string{true: "reconciliation_manual", false: "none"}[expectedUncertain] {
		t.Fatalf("authenticated CLI session Runner receipt observation=%#v err=%v stdout=%q", observation, err, output)
	}
	cliRequests := recorder.snapshot()[firstCLIRequest:]
	assertNoDeviceOrDispatchRequests(t, cliRequests)
	if len(cliRequests) != 1 || cliRequests[0] != (recordedConversationRequest{method: http.MethodPost, path: previewPath}) {
		t.Fatalf("session Runner receipt CLI requests=%#v", cliRequests)
	}

	firstTUIRequest := len(recorder.snapshot())
	runSessionRunnerReceiptTUI(t, executable, apiURL, accessToken, conversationID, promptID, runID, inputPath,
		expectedDisposition, expectedUncertain)
	tuiRequests := recorder.snapshot()[firstTUIRequest:]
	assertNoDeviceOrDispatchRequests(t, tuiRequests)
	wantTUI := []recordedConversationRequest{
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/prompts", query: "limit=128"},
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/runs", query: "limit=25"},
		{method: http.MethodPost, path: previewPath},
	}
	if len(tuiRequests) != len(wantTUI) {
		t.Fatalf("session Runner receipt TUI issued %d requests; want %#v got %#v", len(tuiRequests), wantTUI, tuiRequests)
	}
	for index, request := range tuiRequests {
		if request != wantTUI[index] {
			t.Fatalf("session Runner receipt TUI request[%d]=%#v want=%#v", index, request, wantTUI[index])
		}
	}
}

func assertNoDeviceOrDispatchRequests(t *testing.T, requests []recordedConversationRequest) {
	t.Helper()
	for _, request := range requests {
		if strings.Contains(request.path, "/devices") ||
			strings.Contains(request.path, "dispatch") ||
			strings.Contains(request.path, "execution") {
			t.Fatalf("receipt observation surface issued forbidden device/dispatch request: %#v", request)
		}
	}
}

func marshalSessionRunnerReceiptObservation(
	t *testing.T,
	owner deviceplacement.Owner,
	conversationID, promptID, runID string,
	intent deviceplacement.RunnerExecutionIntentObservation,
	command deviceplacement.RunnerTerminalCommand,
	proof deviceplacement.RunnerTerminalLeaseProof,
	commandSHA256 string,
	disposition deviceplacement.RunnerTerminalDisposition,
) ([]byte, error) {
	t.Helper()
	receiptObservation, err := deviceplacement.ObserveRunnerTerminalReceipt(
		deviceplacement.RunnerTerminalReceiptRequest{
			Grant: deviceplacement.RunnerTerminalLeaseGrant{
				V: 1, AttemptID: proof.AttemptID, TargetID: proof.TargetID,
				Epoch: proof.Epoch, FencingToken: proof.FencingToken,
				IssuedAtMS: 100, ExpiresAtMS: 2000,
			},
			Command: command,
			Receipt: deviceplacement.RunnerTerminalReceipt{
				V: 1, CommandID: command.CommandID, CommandSHA256: commandSHA256,
				Proof:        proof,
				Disposition:  disposition,
				ObservedAtMS: 300,
			},
		},
	)
	uncertain := disposition.Kind == "uncertain"
	if err != nil || !receiptObservation.ReceiptValid ||
		!receiptObservation.PreviewOnly || receiptObservation.Uncertain != uncertain ||
		receiptObservation.ReconciliationRequired != uncertain ||
		receiptObservation.ManualReviewRequired != uncertain ||
		receiptObservation.AutomaticRetry ||
		receiptObservation.FollowUp != map[bool]string{true: "reconciliation_manual", false: "none"}[uncertain] ||
		receiptObservation.Authority != (deviceplacement.RunnerTerminalReceiptAuthority{}) {
		t.Fatalf("Runner terminal receipt observation=%#v err=%v", receiptObservation, err)
	}
	sessionObservation, err := deviceplacement.ObserveSessionRunnerReceipt(
		deviceplacement.SessionRunnerReceiptObservationRequest{
			Owner: owner, ConversationID: conversationID, PromptID: promptID,
			RunID: runID, Intent: intent, Receipt: receiptObservation,
		},
	)
	if err != nil || !sessionObservation.PromptRunBindingValid ||
		!sessionObservation.ReceiptBindingValid || !sessionObservation.PreviewOnly ||
		sessionObservation.SelectedTargetID != nil ||
		sessionObservation.Authority != (deviceplacement.SessionRunnerReceiptAuthority{}) {
		t.Fatalf("session Runner receipt observation=%#v err=%v", sessionObservation, err)
	}
	return json.Marshal(sessionObservation)
}
