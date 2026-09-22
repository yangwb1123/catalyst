package appserver

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

type localRunnerPreviewRouteFake struct {
	output string
	exit   int
	err    error
	calls  int
	argv   []string
}

func (fake *localRunnerPreviewRouteFake) Run(
	_ context.Context, argv []string, stdin string, timeout time.Duration,
) (string, int, error) {
	if stdin != "" || timeout != 5*time.Second {
		return "", 0, context.Canceled
	}
	fake.calls++
	fake.argv = append([]string(nil), argv...)
	return fake.output, fake.exit, fake.err
}

func TestLocalRunnerPreviewCandidateBindsOwnerAndInvokesInjectedRunner(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	fake := &localRunnerPreviewRouteFake{output: "private executor output"}
	sessions := authenticator.Handler(newConversationRoutesWithInertExecutionAPIAndLocalRunnerPreview(
		nil, nil, &localRunnerPreviewCandidateConfig{
			Enabled: true,
			Adapter: deviceplacement.LocalRunnerPreviewAdapter{Executor: fake},
		},
	))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	request := localRunnerPreviewRouteRequest(t, identity.issuer, "account-42", "tenant-slate")
	bodyBytes, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	path := "/api/v1/conversations/conversation-001/run-intents/intent-001/execution-readiness-preview"
	response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		localRunnerPreviewScope, "application/json", "", string(bodyBytes))
	if response.Code != http.StatusOK {
		t.Fatalf("local Runner preview status=%d body=%q", response.Code, response.Body.String())
	}
	var observation deviceplacement.LocalRunnerPreviewObservation
	if err := json.Unmarshal(response.Body.Bytes(), &observation); err != nil {
		t.Fatalf("decode local Runner preview: %v body=%q", err, response.Body.String())
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate local Runner preview: %v", err)
	}
	if fake.calls != 1 || strings.Join(fake.argv, " ") != "forge-task --prompt-ref prompt-001" {
		t.Fatalf("injected Runner calls=%d argv=%#v", fake.calls, fake.argv)
	}
	if observation.Intent.Owner.Subject != "account-42" || observation.Intent.ConversationID != "conversation-001" ||
		observation.Intent.TargetID != "runner-1" || observation.SessionReceipt.SelectedTargetID != nil ||
		observation.Authority != (deviceplacement.LocalRunnerPreviewAuthority{}) {
		t.Fatalf("local Runner preview binding/authority=%#v", observation)
	}
	if bytes.Contains(response.Body.Bytes(), []byte("private executor output")) || bytes.Contains(response.Body.Bytes(), []byte("fence-001")) {
		t.Fatalf("local Runner output or lease token leaked: %s", response.Body.Bytes())
	}
	repeat := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		localRunnerPreviewScope, "application/json", "", string(bodyBytes))
	if repeat.Code != http.StatusOK || !bytes.Equal(response.Body.Bytes(), repeat.Body.Bytes()) {
		t.Fatalf("repeated local Runner preview changed: first=%q second=%q", response.Body.String(), repeat.Body.String())
	}
}

func TestLocalRunnerPreviewCandidateFailsClosedAtHTTPBoundary(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	fake := &localRunnerPreviewRouteFake{output: "must not execute"}
	config := &localRunnerPreviewCandidateConfig{
		Enabled: true,
		Adapter: deviceplacement.LocalRunnerPreviewAdapter{Executor: fake},
	}
	sessions := authenticator.Handler(newConversationRoutesWithInertExecutionAPIAndLocalRunnerPreview(nil, nil, config))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	request := localRunnerPreviewRouteRequest(t, identity.issuer, "account-42", "tenant-slate")
	bodyBytes, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	path := "/api/v1/conversations/conversation-001/run-intents/intent-001/execution-readiness-preview"
	if response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		"forge:conversations:write", "application/json", "", string(bodyBytes)); response.Code != http.StatusForbidden {
		t.Fatalf("missing read scope status=%d body=%q", response.Code, response.Body.String())
	}
	if response := requestConversationAPI(t, routes, identity, http.MethodGet, path,
		localRunnerPreviewScope, "", "", ""); response.Code != http.StatusMethodNotAllowed || response.Header().Get("Allow") != http.MethodPost {
		t.Fatalf("method status=%d allow=%q body=%q", response.Code, response.Header().Get("Allow"), response.Body.String())
	}
	if response := requestConversationAPI(t, routes, identity, http.MethodPost, path+"?debug=1",
		localRunnerPreviewScope, "application/json", "", string(bodyBytes)); response.Code != http.StatusBadRequest {
		t.Fatalf("query status=%d body=%q", response.Code, response.Body.String())
	}
	foreign := requestConversationAPIAs(t, routes, identity, http.MethodPost, path,
		localRunnerPreviewScope, "other-account", "tenant-slate", "application/json", "", string(bodyBytes))
	if foreign.Code != http.StatusForbidden || fake.calls != 0 {
		t.Fatalf("foreign owner status=%d calls=%d body=%q", foreign.Code, fake.calls, foreign.Body.String())
	}
	wrongPath := requestConversationAPI(t, routes, identity, http.MethodPost,
		"/api/v1/conversations/other-conversation/run-intents/intent-001/execution-readiness-preview",
		localRunnerPreviewScope, "application/json", "", string(bodyBytes))
	if wrongPath.Code != http.StatusBadRequest || fake.calls != 0 {
		t.Fatalf("wrong path status=%d calls=%d body=%q", wrongPath.Code, fake.calls, wrongPath.Body.String())
	}
	unknown := strings.Replace(string(bodyBytes), `"observed_at_ms":300`, `"observed_at_ms":300,"unexpected":true`, 1)
	if response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		localRunnerPreviewScope, "application/json", "", unknown); response.Code != http.StatusBadRequest {
		t.Fatalf("unknown field status=%d body=%q", response.Code, response.Body.String())
	}
	duplicate := strings.Replace(string(bodyBytes), `"observed_at_ms":300`, `"observed_at_ms":300,"observed_at_ms":300`, 1)
	if response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		localRunnerPreviewScope, "application/json", "", duplicate); response.Code != http.StatusBadRequest {
		t.Fatalf("duplicate field status=%d body=%q", response.Code, response.Body.String())
	}
	lease := request
	lease.Grant.FencingToken = "foreign-fence"
	leaseBody, err := json.Marshal(lease)
	if err != nil {
		t.Fatal(err)
	}
	if response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		localRunnerPreviewScope, "application/json", "", string(leaseBody)); response.Code != http.StatusBadRequest || fake.calls != 0 {
		t.Fatalf("lease mismatch status=%d calls=%d body=%q", response.Code, fake.calls, response.Body.String())
	}
}

func TestLocalRunnerPreviewCandidatePreservesFailureAndUncertaintyMetadata(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	fake := &localRunnerPreviewRouteFake{}
	sessions := authenticator.Handler(newConversationRoutesWithInertExecutionAPIAndLocalRunnerPreview(
		nil, nil, &localRunnerPreviewCandidateConfig{
			Enabled: true, Adapter: deviceplacement.LocalRunnerPreviewAdapter{Executor: fake},
		},
	))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	request := localRunnerPreviewRouteRequest(t, identity.issuer, "account-42", "tenant-slate")
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	path := "/api/v1/conversations/conversation-001/run-intents/intent-001/execution-readiness-preview"
	tests := []struct {
		name        string
		output      string
		exit        int
		execErr     error
		disposition string
		uncertain   bool
		leak        string
	}{
		{name: "failed", output: "private diagnostic", exit: 7, disposition: "failed", leak: "private diagnostic"},
		{name: "uncertain", execErr: errors.New("private transport detail"), disposition: "uncertain", uncertain: true, leak: "private transport detail"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			fake.output, fake.exit, fake.err = test.output, test.exit, test.execErr
			response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
				localRunnerPreviewScope, "application/json", "", string(body))
			if response.Code != http.StatusOK {
				t.Fatalf("preview status=%d body=%q", response.Code, response.Body.String())
			}
			var observation deviceplacement.LocalRunnerPreviewObservation
			if err := json.Unmarshal(response.Body.Bytes(), &observation); err != nil {
				t.Fatal(err)
			}
			receipt := observation.SessionReceipt.ReceiptObservation
			if err := observation.Validate(); err != nil || observation.DispositionKind != test.disposition || receipt.Uncertain != test.uncertain {
				t.Fatalf("observation=%#v receipt=%#v err=%v", observation, receipt, err)
			}
			if strings.Contains(response.Body.String(), test.leak) {
				t.Fatalf("private executor detail leaked: %q", response.Body.String())
			}
		})
	}
}

func TestValidLocalRunnerPreviewObservationRejectsIdentityDrift(t *testing.T) {
	request := localRunnerPreviewRouteRequest(t, "https://id.example", "account-42", "tenant-slate")
	observation, err := (deviceplacement.LocalRunnerPreviewAdapter{
		Executor: &localRunnerPreviewRouteFake{},
	}).Execute(context.Background(), request)
	if err != nil {
		t.Fatal(err)
	}
	owner := model.Owner{Issuer: "https://id.example", Subject: "account-42", TenantID: "tenant-slate"}
	mutations := []struct {
		name   string
		change func(*deviceplacement.LocalRunnerPreviewObservation)
	}{
		{name: "prompt", change: func(value *deviceplacement.LocalRunnerPreviewObservation) { value.Intent.PromptID = "prompt-foreign" }},
		{name: "run", change: func(value *deviceplacement.LocalRunnerPreviewObservation) { value.Intent.RunID = "run-foreign" }},
		{name: "attempt", change: func(value *deviceplacement.LocalRunnerPreviewObservation) { value.AttemptID = "attempt-foreign" }},
		{name: "command", change: func(value *deviceplacement.LocalRunnerPreviewObservation) { value.CommandID = "command-foreign" }},
		{name: "digest", change: func(value *deviceplacement.LocalRunnerPreviewObservation) {
			value.CommandSHA256 = strings.Repeat("a", 64)
		}},
		{name: "time", change: func(value *deviceplacement.LocalRunnerPreviewObservation) { value.ObservedAtMS++ }},
	}
	for _, test := range mutations {
		t.Run(test.name, func(t *testing.T) {
			mutated := observation
			test.change(&mutated)
			if validLocalRunnerPreviewObservation(mutated, owner, "conversation-001", request) {
				t.Fatal("identity-drifted observation was accepted")
			}
		})
	}
}

func TestLocalRunnerPreviewCandidateRemainsUnmountedFromProductionSessionRoutes(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	request := localRunnerPreviewRouteRequest(t, identity.issuer, "account-42", "tenant-slate")
	body, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	response := requestConversationAPI(t, routes, identity, http.MethodPost,
		"/api/v1/conversations/conversation-001/run-intents/intent-001/execution-readiness-preview",
		localRunnerPreviewScope, "application/json", "", string(body))
	if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
		t.Fatalf("production local Runner preview status=%d body=%q", response.Code, response.Body.String())
	}
}

func localRunnerPreviewRouteRequest(t *testing.T, issuer, subject, tenant string) deviceplacement.LocalRunnerPreviewRequest {
	t.Helper()
	command := deviceplacement.RunnerExecutionCommand{
		V: 1, CommandID: "command-001",
		LeaseProof: deviceplacement.RunnerExecutionLeaseProof{
			AttemptID: "attempt-001", TargetID: "runner-1", Epoch: 1, FencingToken: "fence-001",
		},
		IdempotencyKey: "run-001:attempt-001:command-001", WorkspaceRef: "workspace-001",
		Argv: []string{"forge-task", "--prompt-ref", "prompt-001"}, TimeoutMS: 5_000, MaxOutputBytes: 65_536,
	}
	digest, err := (deviceplacement.RunnerTerminalCommand{
		V: command.V, CommandID: command.CommandID,
		LeaseProof: deviceplacement.RunnerTerminalLeaseProof{
			AttemptID: command.LeaseProof.AttemptID, TargetID: command.LeaseProof.TargetID,
			Epoch: command.LeaseProof.Epoch, FencingToken: command.LeaseProof.FencingToken,
		},
		IdempotencyKey: command.IdempotencyKey, WorkspaceRef: command.WorkspaceRef,
		Argv: command.Argv, TimeoutMS: command.TimeoutMS, MaxOutputBytes: command.MaxOutputBytes,
	}).CommandSHA256()
	if err != nil {
		t.Fatal(err)
	}
	owner := deviceplacement.Owner{Issuer: issuer, Subject: subject, TenantID: tenant}
	return deviceplacement.LocalRunnerPreviewRequest{
		Intent: deviceplacement.RunnerExecutionIntentRequest{
			Owner: owner, ConversationID: "conversation-001",
			Prompt: deviceplacement.RunIntentPromptReceipt{
				PromptID: "prompt-001", ConversationID: "conversation-001", Role: "user", AcceptedAtMS: 100,
				IntentID: "intent-001", InitialEventID: "event-001", InitialEventSequence: 1, InitialEventType: "submitted",
			},
			Run: deviceplacement.RunIntentRunReference{
				RunID: "run-001", ConversationID: "conversation-001", PromptID: "prompt-001", CreatedAtMS: 100,
				LatestSequence: 1, Status: "nonterminal",
			},
			Binding: deviceplacement.RunnerExecutionIntentBinding{
				ConversationID: "conversation-001", PromptID: "prompt-001", RunID: "run-001", AttemptID: "attempt-001",
				CommandID: "command-001", TargetID: "runner-1", CommandSHA256: digest,
				IdempotencyKey: "run-001:attempt-001:command-001",
			},
			Command: command,
		},
		Grant: deviceplacement.RunnerTerminalLeaseGrant{
			V: 1, AttemptID: "attempt-001", TargetID: "runner-1", Epoch: 1, FencingToken: "fence-001",
			IssuedAtMS: 100, ExpiresAtMS: 10_100,
		},
		ObservedAtMS: 300,
	}
}
