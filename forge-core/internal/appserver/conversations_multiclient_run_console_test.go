package appserver

import (
	"context"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"
)

// runForgeConsoleRunObservationAPITest drives the real Flutter API client
// against a populated Run. It intentionally has no Prompt write path: the
// only write is the caller-supplied, stateless P3a device-observation preview.
func runForgeConsoleRunObservationAPITest(
	t *testing.T, apiURL, token, issuer, conversationID, runID,
	sessionObservationRequest string,
	sessionObservationResponse json.RawMessage,
	runObserved json.RawMessage,
	sessionRunnerReceiptObservation json.RawMessage,
) {
	t.Helper()
	runForgeConsoleRunObservationFlutterTest(
		t, apiURL, token, conversationID, runID,
		"test/forge_run_observation_live_e2e_test.dart",
		"FORGE_RUN_OBSERVATION_E2E_INPUT", "API",
		multiInstanceDevicePlacementPreviewBody(
			t, issuer, snaplinkForgeTestUser, snaplinkForgeTestTenant,
		),
		sessionObservationRequest, sessionObservationResponse, runObserved, nil, nil,
		sessionRunnerReceiptObservation,
	)
}

func runForgeConsoleRunObservationWidgetE2ETest(
	t *testing.T, apiURL, token, conversationID, runID string,
	runObserved json.RawMessage,
	sessionObservationRequest string,
	runIntentObservation json.RawMessage,
	runnerExecutionObservation json.RawMessage,
	sessionRunnerReceiptObservation json.RawMessage,
) {
	t.Helper()
	runForgeConsoleRunObservationFlutterTest(
		t, apiURL, token, conversationID, runID,
		"test/forge_run_observation_widget_live_e2e_test.dart",
		"FORGE_RUN_OBSERVATION_WIDGET_E2E_INPUT", "native widget", "",
		sessionObservationRequest, nil, runObserved, runIntentObservation, runnerExecutionObservation,
		sessionRunnerReceiptObservation,
	)
}

func runForgeConsoleRunObservationFlutterTest(
	t *testing.T,
	apiURL, token, conversationID, runID, testFile, inputEnvironment, label string,
	placementRequest string,
	sessionObservationRequest string,
	sessionObservationResponse json.RawMessage,
	runObserved json.RawMessage,
	runIntentObservation json.RawMessage,
	runnerExecutionObservation json.RawMessage,
	sessionRunnerReceiptObservation json.RawMessage,
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
		t.Fatalf("Flutter is required when FORGE_CONSOLE_E2E=1: %v", err)
	}

	inputJSON, err := json.Marshal(struct {
		APIURL                     string          `json:"api_url"`
		AccessToken                string          `json:"access_token"`
		ConversationID             string          `json:"conversation_id"`
		RunID                      string          `json:"run_id"`
		Placement                  json.RawMessage `json:"placement_request,omitempty"`
		SessionDevice              json.RawMessage `json:"session_device_observation_request,omitempty"`
		SessionObservationResponse json.RawMessage `json:"session_device_observation_response,omitempty"`
		RunObserved                json.RawMessage `json:"run_observed,omitempty"`
		RunIntent                  json.RawMessage `json:"run_intent_observation,omitempty"`
		RunnerExecution            json.RawMessage `json:"runner_execution_intent_observation,omitempty"`
		SessionRunnerReceipt       json.RawMessage `json:"session_runner_receipt_observation,omitempty"`
	}{
		APIURL: apiURL, AccessToken: token, ConversationID: conversationID, RunID: runID,
		Placement: func() json.RawMessage {
			if placementRequest == "" {
				return nil
			}
			return json.RawMessage(placementRequest)
		}(),
		SessionDevice: func() json.RawMessage {
			if sessionObservationRequest == "" {
				return nil
			}
			return json.RawMessage(sessionObservationRequest)
		}(),
		RunIntent: runIntentObservation, RunnerExecution: runnerExecutionObservation,
		SessionRunnerReceipt:       sessionRunnerReceiptObservation,
		SessionObservationResponse: sessionObservationResponse,
		RunObserved:                runObserved,
	})
	if err != nil {
		t.Fatalf("encode Flutter Run observation input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "run-observation-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write private Flutter Run observation input: %v", err)
	}
	inputInfo, err := os.Stat(inputPath)
	if err != nil {
		t.Fatalf("stat private Flutter Run observation input: %v", err)
	}
	if inputInfo.Mode().Perm() != 0o600 {
		t.Fatalf("private Flutter Run observation input mode=%#o want 0600", inputInfo.Mode().Perm())
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create isolated Flutter home: %v", err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable,
		"test", "--no-pub",
		"--dart-define=FORGE_CONVERSATIONS_API_ORIGIN="+apiURL,
		testFile)
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		inputEnvironment+"="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter Run observation %s E2E failed: stdout=%q stderr=%q err=%v",
			label, stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("Flutter Run observation %s output exceeded the size limit", label)
	}
}
