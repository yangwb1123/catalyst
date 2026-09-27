package appserver

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/executionattempt"
	"forgeos/forge-core/internal/executionlease"
)

func TestRunnerAttemptBoundaryPreviewRequiresAcceptedAssemblyAndAuth(t *testing.T) {
	fixture := newRunnerAttemptBoundaryRouteFixture(t)
	body := runnerAttemptBoundaryBody(t, fixture.owner, executionattempt.BeginStarting)
	path := fmt.Sprintf(runnerAttemptBoundaryPath, "conversation-1", "run-1")

	noScope := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, path,
		"forge:conversations:read", "application/json", "", body)
	if noScope.Code != http.StatusForbidden {
		t.Fatalf("wrong scope status=%d body=%q", noScope.Code, noScope.Body.String())
	}
	accepted := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, path,
		runnerTransportAdmissionScope, "application/json", "", body)
	if accepted.Code != http.StatusOK {
		t.Fatalf("accepted attempt boundary status=%d body=%q", accepted.Code, accepted.Body.String())
	}
	var observation deviceplacement.RunnerAttemptBoundaryObservation
	if err := json.Unmarshal(accepted.Body.Bytes(), &observation); err != nil {
		t.Fatal(err)
	}
	if err := observation.Validate(); err != nil || !observation.AttemptBoundaryReady ||
		observation.Authority != (deviceplacement.RunnerAttemptBoundaryAuthority{}) {
		t.Fatalf("attempt boundary observation=%#v err=%v", observation, err)
	}
	if strings.Contains(accepted.Body.String(), "fencing_token") ||
		strings.Contains(accepted.Body.String(), "argv") ||
		strings.Contains(accepted.Body.String(), "payload_body") {
		t.Fatalf("attempt boundary leaked effect material: %q", accepted.Body.String())
	}
}

func TestRunnerAttemptBoundaryPreviewRejectsStaleLeaseAfterRenewal(t *testing.T) {
	fixture := newRunnerAttemptBoundaryRouteFixture(t)
	path := fmt.Sprintf(runnerAttemptBoundaryPath, "conversation-1", "run-1")
	staleBody := runnerAttemptBoundaryBody(t, fixture.owner, executionattempt.BeginStarting)

	leaseAdapter, err := deviceinventory.NewPersistedExecutionLeaseRegistryFileSetWriteAdapterWithTokenSource(
		fixture.leasePath, fixture.owner, func() (string, error) { return "token-b", nil },
	)
	if err != nil {
		t.Fatal(err)
	}
	renewed, replayed, err := leaseAdapter.Renew(context.Background(), executionlease.RenewRequest{
		ConversationID: fixture.entry.ConversationID, RunID: fixture.entry.RunID, AttemptID: fixture.entry.AttemptID,
		Proof: fixture.entry.Grant.Proof(), IdempotencyKey: "attempt-boundary-renew-000001",
		RequestSHA256: executionlease.RequestDigest([]byte("renew")), IssuedAtMS: uint64(time.Now().UnixMilli()), TTLMS: 30_000,
	})
	if err != nil || replayed || renewed.Grant.Epoch != fixture.entry.Grant.Epoch+1 ||
		renewed.Grant.FencingToken == fixture.entry.Grant.FencingToken {
		t.Fatalf("renewed=%#v replayed=%v entry=%#v err=%v", renewed, replayed, fixture.entry, err)
	}

	staleResponse := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, path,
		runnerTransportAdmissionScope, "application/json", "", staleBody)
	if staleResponse.Code != http.StatusConflict || !strings.Contains(staleResponse.Body.String(), `"code":"lease_stale"`) {
		t.Fatalf("stale attempt boundary status=%d body=%q", staleResponse.Code, staleResponse.Body.String())
	}
}

func TestRunnerAttemptBoundaryPreviewRejectsBindingAndWireDrift(t *testing.T) {
	fixture := newRunnerAttemptBoundaryRouteFixture(t)
	path := fmt.Sprintf(runnerAttemptBoundaryPath, "conversation-1", "run-1")
	valid := runnerAttemptBoundaryBody(t, fixture.owner, executionattempt.BeginStarting)
	mutations := []struct {
		name string
		body string
		code int
	}{
		{name: "path binding", body: replaceJSONField(valid, "run_id", `"run-other"`), code: http.StatusBadRequest},
		{name: "unknown field", body: appendJSONField(valid, "unexpected", `true`), code: http.StatusBadRequest},
		{name: "duplicate field", body: duplicateJSONField(valid, "transition", `"begin_starting"`), code: http.StatusBadRequest},
		{name: "trailing value", body: valid + "{}", code: http.StatusBadRequest},
		{name: "authority injection", body: appendJSONField(valid, "authority", `{}`), code: http.StatusBadRequest},
		{name: "unknown transition", body: replaceJSONField(valid, "transition", `"unknown_transition"`), code: http.StatusBadRequest},
		{name: "owner binding", body: replaceJSONField(valid, "owner", `{"issuer":"https://foreign.example","subject":"other","tenant_id":"tenant-slate"}`), code: http.StatusForbidden},
	}
	for _, mutation := range mutations {
		t.Run(mutation.name, func(t *testing.T) {
			response := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, path,
				runnerTransportAdmissionScope, "application/json", "", mutation.body)
			if response.Code != mutation.code {
				t.Fatalf("status=%d body=%q, want %d", response.Code, response.Body.String(), mutation.code)
			}
		})
	}
}

func TestRunnerAttemptBoundaryPreviewReusesLifecycleAndAuthorityGates(t *testing.T) {
	fixture := newRunnerAttemptBoundaryRouteFixture(t)
	path := fmt.Sprintf(runnerAttemptBoundaryPath, "conversation-1", "run-1")
	invalidLifecycle := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, path,
		runnerTransportAdmissionScope, "application/json", "",
		runnerAttemptBoundaryBody(t, fixture.owner, executionattempt.ObserveRunning))
	if invalidLifecycle.Code != http.StatusOK {
		t.Fatalf("invalid lifecycle status=%d body=%q", invalidLifecycle.Code, invalidLifecycle.Body.String())
	}
	var observation deviceplacement.RunnerAttemptBoundaryObservation
	if err := json.Unmarshal(invalidLifecycle.Body.Bytes(), &observation); err != nil {
		t.Fatal(err)
	}
	if observation.AttemptBoundaryReady || observation.AttemptTransitionValid ||
		!containsAttemptBoundaryReason(observation.RejectionReasons, "attempt_transition_invalid") {
		t.Fatalf("invalid lifecycle became dispatchable: %#v", observation)
	}

	withoutAuthority := newRunnerAttemptBoundaryRouteFixtureWithoutAuthority(t)
	closed := requestConversationAPI(t, withoutAuthority.handler, withoutAuthority.identity, http.MethodPost, path,
		runnerTransportAdmissionScope, "application/json", "", runnerAttemptBoundaryBody(t, withoutAuthority.owner, executionattempt.BeginStarting))
	if closed.Code != http.StatusNotFound {
		t.Fatalf("authority-free route status=%d body=%q", closed.Code, closed.Body.String())
	}
	nonExecute := newRunnerAttemptBoundaryRouteFixtureNonExecute(t)
	closed = requestConversationAPI(t, nonExecute.handler, nonExecute.identity, http.MethodPost, path,
		runnerTransportAdmissionScope, "application/json", "", runnerAttemptBoundaryBody(t, nonExecute.owner, executionattempt.BeginStarting))
	if closed.Code != http.StatusNotFound {
		t.Fatalf("non-EXECUTE route status=%d body=%q", closed.Code, closed.Body.String())
	}
}

type runnerAttemptBoundaryRouteFixture struct {
	identity  *conversationTestIdentity
	handler   http.Handler
	owner     deviceidentity.Owner
	leasePath string
	entry     executionlease.RegistryEntry
}

func newRunnerAttemptBoundaryRouteFixture(t *testing.T) runnerAttemptBoundaryRouteFixture {
	t.Helper()
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{
		lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1),
	})
	leasePath := filepath.Join(filepath.Dir(registryPath), "leases.json")
	leaseAdapter, err := deviceinventory.NewPersistedExecutionLeaseRegistryFileSetWriteAdapterWithTokenSource(
		leasePath, owner, func() (string, error) { return "token-a", nil },
	)
	if err != nil {
		t.Fatal(err)
	}
	entry, replayed, err := leaseAdapter.Claim(context.Background(), executionlease.ClaimRequest{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		IdempotencyKey: "attempt-boundary-claim-000001", RequestSHA256: executionlease.RequestDigest([]byte("claim")),
		IssuedAtMS: uint64(time.Now().UnixMilli()) - 1_000, TTLMS: 30_000,
	}, []executionlease.ClaimCandidate{{DeviceID: "device-a", InstanceID: "runner-a", Revision: 1, Generation: 1, HeartbeatSequence: 1}})
	if err != nil || replayed || entry.Grant.Epoch == 0 {
		t.Fatalf("lease entry=%#v replayed=%v err=%v", entry, replayed, err)
	}
	activation := acceptedRunnerAttemptBoundaryActivation()
	authority := devicefabricgate.RunnerAuthorityConfig{
		Enabled: true, AuthorityID: "runner-authority-attempt-boundary",
		Decision: devicefabricgate.Decision{Status: "accepted", AcceptanceID: "runner-authority-attempt-boundary-001", AcceptedAtUnixMS: 1},
	}
	sessions, err := newAuthenticatedSessionRoutesWithDeviceFabricActivationAndRunnerAuthority(
		nil, nil, ptrDeviceFabricRequest(activation), registryPath, "", &authority, leasePath,
	)
	if err != nil {
		t.Fatal(err)
	}
	return runnerAttemptBoundaryRouteFixture{
		identity: identity, handler: authenticator.Handler(sessions), owner: owner,
		leasePath: leasePath, entry: entry,
	}
}

func newRunnerAttemptBoundaryRouteFixtureWithoutAuthority(t *testing.T) runnerAttemptBoundaryRouteFixture {
	t.Helper()
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{
		lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1),
	})
	activation := acceptedRunnerAttemptBoundaryActivation()
	sessions, err := newAuthenticatedSessionRoutesWithDeviceFabricActivation(
		nil, nil, ptrDeviceFabricRequest(activation), registryPath, "",
	)
	if err != nil {
		t.Fatal(err)
	}
	return runnerAttemptBoundaryRouteFixture{identity: identity, handler: authenticator.Handler(sessions), owner: owner}
}

func newRunnerAttemptBoundaryRouteFixtureNonExecute(t *testing.T) runnerAttemptBoundaryRouteFixture {
	t.Helper()
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{
		lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1),
	})
	activation := acceptedInventoryActivation()
	sessions, err := newAuthenticatedSessionRoutesWithDeviceFabricActivation(
		nil, nil, ptrDeviceFabricRequest(activation), registryPath, "",
	)
	if err != nil {
		t.Fatal(err)
	}
	return runnerAttemptBoundaryRouteFixture{identity: identity, handler: authenticator.Handler(sessions), owner: owner}
}

func acceptedRunnerAttemptBoundaryActivation() devicefabricgate.Request {
	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{Status: "accepted", AcceptanceID: "p4-attempt-boundary-001", AcceptedAtUnixMS: 1}
	activation.Evidence.RunnerIsolation = true
	activation.Evidence.LeaseFencing = true
	activation.Evidence.CancellationAndUncertainWork = true
	activation.Evidence.VaultArtifactAuthorization = true
	activation.Evidence.AuditOutbox = true
	return activation
}

func runnerAttemptBoundaryBody(t *testing.T, owner deviceidentity.Owner, transition executionattempt.Transition) string {
	t.Helper()
	transport := runnerTransportAdmissionObservation(t)
	request := deviceplacement.RunnerAttemptBoundaryPreviewRequest{
		RunnerExecutionBoundaryPreviewRequest: deviceplacement.RunnerExecutionBoundaryPreviewRequest{
			Owner: ownerToPlacement(owner), ConversationID: "conversation-1", RunID: "run-1",
			AttemptID: "attempt-1", AttemptState: "accepted", Command: runnerDispatchAdmissionIntent(t),
			Transport: transport, ExpectedPayloadSHA256: transport.PayloadSHA256,
			Controls: deviceplacement.RunnerExecutionBoundaryControls{EffectState: "not_started"},
		},
		Transition: transition,
	}
	encoded, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	return string(encoded)
}

func containsAttemptBoundaryReason(values []string, want string) bool {
	for _, value := range values {
		if value == want {
			return true
		}
	}
	return false
}

func replaceJSONField(body, field, value string) string {
	var object map[string]json.RawMessage
	if err := json.Unmarshal([]byte(body), &object); err != nil {
		return body
	}
	object[field] = json.RawMessage(value)
	encoded, err := json.Marshal(object)
	if err != nil {
		return body
	}
	return string(encoded)
}

func appendJSONField(body, field, value string) string {
	return strings.TrimSuffix(body, "}") + fmt.Sprintf(",\"%s\":%s}", field, value)
}

func duplicateJSONField(body, field, value string) string {
	return strings.TrimSuffix(body, "}") + fmt.Sprintf(",\"%s\":%s}", field, value)
}
