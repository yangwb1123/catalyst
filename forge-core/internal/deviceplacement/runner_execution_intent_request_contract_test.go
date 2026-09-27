package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"testing"
)

func TestRunnerExecutionIntentRequestContractFixture(t *testing.T) {
	request := readRunnerExecutionIntentRequestFixture(t)
	digest, err := request.Command.commandSHA256()
	if err != nil {
		t.Fatalf("compute Runner command digest: %v", err)
	}
	if digest != request.Binding.CommandSHA256 ||
		request.Binding.IdempotencyKey != request.Run.RunID+":"+request.Binding.AttemptID+":"+request.Binding.CommandID {
		t.Fatalf("Runner execution-intent request has inconsistent command binding: %#v", request)
	}
	observation, err := ObserveRunnerExecutionIntent(request)
	if err != nil {
		t.Fatalf("observe canonical Runner execution-intent request: %v", err)
	}
	if observation.Owner != request.Owner ||
		observation.ConversationID != request.ConversationID ||
		observation.PromptID != request.Prompt.PromptID ||
		observation.RunID != request.Run.RunID ||
		observation.AttemptID != request.Binding.AttemptID ||
		observation.CommandID != request.Binding.CommandID ||
		observation.TargetID != request.Binding.TargetID ||
		observation.CommandSHA256 != digest ||
		!observation.PromptRunBindingValid ||
		!observation.RunnerCommandBindingValid ||
		!observation.PreviewOnly ||
		observation.SelectedTargetID != nil ||
		!validRunnerAuthority(observation.Authority) {
		t.Fatalf("unexpected Runner execution-intent observation: %#v", observation)
	}
}

func TestRunnerExecutionIntentRequestFixtureRejectsUnknownFields(t *testing.T) {
	path := os.Getenv("FORGE_RUNNER_EXECUTION_INTENT_REQUEST_FIXTURE")
	if path == "" {
		t.Skip("FORGE_RUNNER_EXECUTION_INTENT_REQUEST_FIXTURE is set by the cross-repository contract test")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	unknown := bytes.Replace(
		encoded,
		[]byte("\"owner\":"),
		[]byte("\"unexpected\":true,\"owner\":"),
		1,
	)
	if _, err := decodeRunnerExecutionIntentRequestFixture(unknown); err == nil {
		t.Fatal("unknown Runner execution-intent request field was accepted")
	}
}

func readRunnerExecutionIntentRequestFixture(t *testing.T) RunnerExecutionIntentRequest {
	t.Helper()
	path := os.Getenv("FORGE_RUNNER_EXECUTION_INTENT_REQUEST_FIXTURE")
	if path == "" {
		t.Skip("FORGE_RUNNER_EXECUTION_INTENT_REQUEST_FIXTURE is set by the cross-repository contract test")
	}
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	request, err := decodeRunnerExecutionIntentRequestFixture(encoded)
	if err != nil {
		t.Fatalf("decode Runner execution-intent request fixture: %v", err)
	}
	return request
}

func decodeRunnerExecutionIntentRequestFixture(encoded []byte) (RunnerExecutionIntentRequest, error) {
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	var request RunnerExecutionIntentRequest
	if err := decoder.Decode(&request); err != nil {
		return RunnerExecutionIntentRequest{}, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return RunnerExecutionIntentRequest{}, errInvalidRequest
		}
		return RunnerExecutionIntentRequest{}, err
	}
	return request, nil
}
