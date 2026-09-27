//go:build linux && !android

package appserver

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceplacement"
)

func postSchedulerSelectionLeaseRenewal(
	t *testing.T,
	apiURL, token string,
	request schedulerSelectionLeaseRenewalRequest,
	idempotencyKey string,
) schedulerSelectionLeaseResponse {
	t.Helper()
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	httpRequest, err := http.NewRequest(
		http.MethodPost, apiURL+schedulerSelectionLeaseRenewalPath, strings.NewReader(string(body)),
	)
	if err != nil {
		t.Fatal(err)
	}
	httpRequest.Header.Set("Authorization", "Bearer "+token)
	httpRequest.Header.Set("Content-Type", "application/json")
	httpRequest.Header.Set("Idempotency-Key", idempotencyKey)
	response, err := (&http.Client{Timeout: 5 * time.Second}).Do(httpRequest)
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	payload, err := io.ReadAll(response.Body)
	if err != nil {
		t.Fatal(err)
	}
	if response.StatusCode != http.StatusOK {
		t.Fatalf("scheduler lease cross-client renewal status=%d body=%q", response.StatusCode, payload)
	}
	var renewed schedulerSelectionLeaseResponse
	if err := json.Unmarshal(payload, &renewed); err != nil {
		t.Fatalf("decode scheduler lease cross-client renewal: %v body=%q", err, payload)
	}
	if renewed.Replayed || renewed.Grant.Epoch != request.Epoch+1 ||
		renewed.Grant.FencingToken == request.FencingToken ||
		renewed.ConversationID != request.ConversationID || renewed.RunID != request.RunID ||
		renewed.AttemptID != request.AttemptID || renewed.InstanceID != request.TargetID ||
		renewed.Authority.ExecutionAuthorized || renewed.Authority.DispatchPerformed || renewed.Authority.AuditPublished {
		t.Fatalf("scheduler lease cross-client renewal=%#v request=%#v", renewed, request)
	}
	return renewed
}

func runForgeConsoleSchedulerSelectionLeaseRenewalE2EWithToken(
	t *testing.T,
	apiURL, token string,
	owner deviceidentity.Owner,
	request schedulerSelectionLeaseRenewalRequest,
	idempotencyKey string,
	expected schedulerSelectionLeaseResponse,
) {
	t.Helper()
	input := struct {
		APIURL           string                                `json:"api_url"`
		AccessToken      string                                `json:"access_token"`
		Owner            deviceplacement.Owner                 `json:"owner"`
		ClientKinds      []string                              `json:"client_kinds"`
		IdempotencyKey   string                                `json:"idempotency_key"`
		RenewalRequest   schedulerSelectionLeaseRenewalRequest `json:"renewal_request"`
		ExpectedDevice   string                                `json:"expected_device_id"`
		ExpectedInstance string                                `json:"expected_instance_id"`
		ExpectedEpoch    uint64                                `json:"expected_epoch"`
	}{
		APIURL: apiURL, AccessToken: token,
		Owner:          deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		ClientKinds:    []string{"web", "app", "mobile"},
		IdempotencyKey: idempotencyKey, RenewalRequest: request,
		ExpectedDevice: expected.DeviceID, ExpectedInstance: expected.InstanceID,
		ExpectedEpoch: expected.Grant.Epoch,
	}
	runForgeConsoleSchedulerLeaseFlutterTest(
		t, "test/forge_scheduler_selection_lease_api_e2e_test.dart",
		"FORGE_SCHEDULER_SELECTION_LEASE_E2E_INPUT", "", input,
		"Flutter scheduler lease renewal E2E",
	)
}

func runForgeConsoleSchedulerSelectionLeaseReleaseGateE2EWithToken(
	t *testing.T,
	apiURL, token string,
	request schedulerSelectionLeaseReleaseRequest,
	idempotencyKey string,
	expected schedulerSelectionLeaseResponse,
) {
	t.Helper()
	input := struct {
		APIURL           string                                `json:"api_url"`
		AccessToken      string                                `json:"access_token"`
		IdempotencyKey   string                                `json:"idempotency_key"`
		ReleaseRequest   schedulerSelectionLeaseReleaseRequest `json:"release_request"`
		ExpectedDevice   string                                `json:"expected_device_id"`
		ExpectedInstance string                                `json:"expected_instance_id"`
		ExpectedEpoch    uint64                                `json:"expected_epoch"`
	}{
		APIURL: apiURL, AccessToken: token, IdempotencyKey: idempotencyKey,
		ReleaseRequest: request, ExpectedDevice: expected.DeviceID,
		ExpectedInstance: expected.InstanceID, ExpectedEpoch: expected.Grant.Epoch,
	}
	runForgeConsoleSchedulerLeaseFlutterTest(
		t, "test/forge_scheduler_selection_lease_lifecycle_gate_e2e_test.dart",
		"FORGE_SCHEDULER_SELECTION_LEASE_LIFECYCLE_GATE_E2E_INPUT", apiURL, input,
		"Flutter scheduler lease lifecycle Gate E2E",
	)
}

func runForgeConsoleSchedulerLeaseFlutterTest(
	t *testing.T,
	testPath, inputEnvironment, apiOrigin string,
	input any,
	label string,
) {
	t.Helper()
	consoleRoot := os.Getenv("SNAPLINK_CONSOLE_ROOT")
	if consoleRoot == "" {
		workingDirectory, err := os.Getwd()
		if err != nil {
			t.Fatalf("resolve Console repository: %v", err)
		}
		repoRoot := filepath.Clean(filepath.Join(workingDirectory, "..", "..", ".."))
		consoleRoot = filepath.Join(filepath.Dir(repoRoot), "workspace", "demo", "snaplink-console")
	}
	if _, err := os.Stat(filepath.Join(consoleRoot, "pubspec.yaml")); err != nil {
		t.Fatalf("SNAPLINK_CONSOLE_ROOT must name the Flutter Console repository: %v", err)
	}
	flutterBinary := os.Getenv("FLUTTER_BIN")
	if flutterBinary == "" {
		flutterBinary = "flutter"
	}
	flutterExecutable, err := exec.LookPath(flutterBinary)
	if err != nil {
		t.Fatalf("Flutter is required for %s: %v", label, err)
	}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode %s input: %v", label, err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "scheduler-selection-lease-lifecycle-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private %s input: %v", label, err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	arguments := []string{"test", "--no-pub"}
	if apiOrigin != "" {
		arguments = append(arguments, "--dart-define=FORGE_CONVERSATIONS_API_ORIGIN="+apiOrigin)
	}
	arguments = append(arguments, testPath)
	command := exec.CommandContext(ctx, flutterExecutable, arguments...)
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		inputEnvironment+"="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("%s failed: stdout=%q stderr=%q err=%v", label, stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("%s output exceeded the size limit", label)
	}
}
