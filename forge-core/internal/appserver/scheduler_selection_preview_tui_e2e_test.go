//go:build linux && !android

package appserver

import (
	"context"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// schedulerSelectionPreviewExpectation keeps the cross-client accepted
// harness explicit about whether a policy-complete preview should select a
// deterministic candidate. It is still only a preview: all authority bits
// must remain false in either branch.
type schedulerSelectionPreviewExpectation struct {
	SelectionAvailable bool
	SelectionReason    string
	DeviceID           string
	InstanceID         string
}

func assertSchedulerSelectionPreviewObservation(
	t *testing.T,
	value deviceplacement.SchedulerSelectionPreviewObservation,
	owner model.Owner,
	request schedulerSelectionPreviewRequest,
	expectation schedulerSelectionPreviewExpectation,
) {
	t.Helper()
	if err := value.Validate(); err != nil || value.Owner != (deviceplacement.Owner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	}) || value.ConversationID != request.ConversationID || value.RunID != request.RunID ||
		value.AttemptID != request.AttemptID || value.SelectionAvailable != expectation.SelectionAvailable ||
		value.SelectionReason != expectation.SelectionReason ||
		value.Authority != (deviceplacement.SchedulerSelectionPreviewAuthority{}) {
		t.Fatalf("authenticated scheduler selection preview=%#v err=%v", value, err)
	}
	if expectation.SelectionAvailable {
		if value.SelectedDeviceID == nil || *value.SelectedDeviceID != expectation.DeviceID ||
			value.SelectedInstanceID == nil || *value.SelectedInstanceID != expectation.InstanceID ||
			value.EligibleCandidateCount == 0 {
			t.Fatalf("authenticated scheduler selection preview selected=%#v expectation=%#v", value, expectation)
		}
		return
	}
	if value.SelectedDeviceID != nil || value.SelectedInstanceID != nil {
		t.Fatalf("authenticated scheduler selection preview unexpectedly selected=%#v", value)
	}
}

func schedulerSelectionPreviewSelectedTarget(
	expectation schedulerSelectionPreviewExpectation,
) string {
	if !expectation.SelectionAvailable {
		return "none"
	}
	return expectation.DeviceID + "/" + expectation.InstanceID
}

// runForgeRuntimeSchedulerSelectionRemoteTUI proves that the accepted
// scheduler-selection observation is consumable by the authenticated TUI.
// The human view deliberately exposes no lease or execution material.
func runForgeRuntimeSchedulerSelectionRemoteTUI(
	t *testing.T,
	executable, apiURL, accessToken string,
	owner model.Owner,
	conversationID, runID string,
) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("authenticated scheduler selection TUI E2E requires script: %v", err)
	}
	var placementRequest devicePlacementRegistryCandidateRequest
	if err := json.Unmarshal([]byte(registryPlacementRequirementsBody(t)), &placementRequest); err != nil {
		t.Fatalf("decode scheduler selection requirements for Runtime TUI: %v", err)
	}
	request := schedulerSelectionPreviewRequest{
		ConversationID: conversationID,
		RunID:          runID,
		AttemptID:      "attempt-1",
		Requirements:   placementRequest.Requirements,
	}
	input, err := json.Marshal(request)
	if err != nil {
		t.Fatalf("encode scheduler selection request for Runtime TUI: %v", err)
	}
	inputPath := filepath.Join(t.TempDir(), "scheduler-selection-requirements.json")
	if err := os.WriteFile(inputPath, input, 0o600); err != nil {
		t.Fatalf("write scheduler selection requirements for Runtime TUI: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader(
		"scheduler-selection-preview --input " + inputPath + "\nquit\n",
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated scheduler selection Rust TUI failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("authenticated scheduler selection Rust TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"scheduler selection preview [forge.scheduler-selection-preview/v1]",
		"owner=" + owner.Issuer + "/" + owner.Subject + " tenant=" + owner.TenantID,
		"conversation=" + conversationID + " run=" + runID + " attempt=attempt-1",
		"selected=none reason=no_eligible_candidate",
		"preview_only=true authority: placement_selected=false reservation_created=false lease_issued=false execution_authorized=false dispatch_performed=false audit_published=false",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("authenticated scheduler selection Rust TUI output omitted %q: %q", want, output)
		}
	}
}

// runForgeRuntimeSchedulerSelectionRemoteCLIWithRequest proves that the
// authenticated Runtime CLI can consume the same policy-complete preview
// request used by the HTTP and Console clients.
func runForgeRuntimeSchedulerSelectionRemoteCLIWithRequest(
	t *testing.T,
	executable, apiURL, accessToken string,
	owner model.Owner,
	request schedulerSelectionPreviewRequest,
	expectation schedulerSelectionPreviewExpectation,
) {
	t.Helper()
	input, err := json.Marshal(request)
	if err != nil {
		t.Fatalf("encode scheduler selection request for Runtime CLI: %v", err)
	}
	inputPath := filepath.Join(t.TempDir(), "scheduler-selection-requirements.json")
	if err := os.WriteFile(inputPath, input, 0o600); err != nil {
		t.Fatalf("write scheduler selection requirements for Runtime CLI: %v", err)
	}
	output, stderr, err := runForgeRuntimeCLI(
		t, executable, apiURL, accessToken, t.TempDir(),
		"--json", "remote", "placement", "scheduler-preview", "--input", inputPath,
	)
	if err != nil {
		t.Fatalf("authenticated scheduler selection Rust CLI failed: stderr=%q stdout=%q err=%v", stderr, output, err)
	}
	var decoded deviceplacement.SchedulerSelectionPreviewObservation
	if err := json.Unmarshal([]byte(output), &decoded); err != nil {
		t.Fatalf("decode authenticated scheduler selection Rust CLI: %v stdout=%q", err, output)
	}
	assertSchedulerSelectionPreviewObservation(t, decoded, owner, request, expectation)
}

// runForgeRuntimeSchedulerSelectionRemoteTUIWithRequest is the human-readable
// counterpart to the CLI helper above. The TUI intentionally renders only
// candidate metadata and the closed authority projection.
func runForgeRuntimeSchedulerSelectionRemoteTUIWithRequest(
	t *testing.T,
	executable, apiURL, accessToken string,
	owner model.Owner,
	request schedulerSelectionPreviewRequest,
	expectation schedulerSelectionPreviewExpectation,
) {
	t.Helper()
	ptyScript, err := exec.LookPath("script")
	if err != nil {
		t.Fatalf("authenticated scheduler selection TUI E2E requires script: %v", err)
	}
	input, err := json.Marshal(request)
	if err != nil {
		t.Fatalf("encode scheduler selection request for Runtime TUI: %v", err)
	}
	inputPath := filepath.Join(t.TempDir(), "scheduler-selection-requirements.json")
	if err := os.WriteFile(inputPath, input, 0o600); err != nil {
		t.Fatalf("write scheduler selection requirements for Runtime TUI: %v", err)
	}
	home := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, ptyScript,
		"-q", "-e", "-f", "/dev/null", "--", executable, "remote", "tui")
	command.Dir = home
	command.Env = forgeRuntimeCLIEnvironment(apiURL, accessToken, home)
	command.Stdin = strings.NewReader(
		"scheduler-selection-preview --input " + inputPath + "\nquit\n",
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("authenticated scheduler selection Rust TUI failed: stdout=%q stderr=%q err=%v",
			stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatal("authenticated scheduler selection Rust TUI output exceeded the size limit")
	}
	output := stdoutBuffer.String()
	for _, want := range []string{
		"scheduler selection preview [forge.scheduler-selection-preview/v1]",
		"owner=" + owner.Issuer + "/" + owner.Subject + " tenant=" + owner.TenantID,
		"conversation=" + request.ConversationID + " run=" + request.RunID + " attempt=" + request.AttemptID,
		"selected=" + schedulerSelectionPreviewSelectedTarget(expectation) + " reason=" + expectation.SelectionReason,
		"preview_only=true authority: placement_selected=false reservation_created=false lease_issued=false execution_authorized=false dispatch_performed=false audit_published=false",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("authenticated scheduler selection Rust TUI output omitted %q: %q", want, output)
		}
	}
}
