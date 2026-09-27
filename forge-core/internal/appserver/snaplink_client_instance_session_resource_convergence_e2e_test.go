package appserver

// This opt-in integration test crosses the real Snaplink JWT boundary and
// reads the two owner-scoped client-instance projections that a client must
// join before using an instance filter.  The projections are mounted only on
// this test mux; no production authority or enrollment path is enabled.

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"testing"
	"time"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
)

func TestSnaplinkAuthenticatedClientInstanceSessionResourceConvergenceE2EWhenConfigured(t *testing.T) {
	if os.Getenv("FORGE_CLIENT_INSTANCE_SESSION_RESOURCE_CONVERGENCE_E2E") != "1" {
		t.Skip("set FORGE_CLIENT_INSTANCE_SESSION_RESOURCE_CONVERGENCE_E2E=1 for paired client-instance E2E")
	}

	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)
	authenticator, err := authn.New(authn.Config{
		Issuer: issuer, Audience: snaplinkForgeTestAudience,
		JWKSURL:          issuer + "/.well-known/jwks.json",
		ExpectedTenantID: snaplinkForgeTestTenant, ExpectedSubjectID: snaplinkForgeTestUser,
		JWKSHTTPClient: ssoClient, JWKSRefreshInterval: 24 * time.Hour,
	})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(authenticator.Close)

	owner := deviceplacement.Owner{
		Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant,
	}
	instances := []deviceplacement.ClientInstanceSessionViewInstance{
		{InstanceID: "client-cli", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{"conversation-a", "conversation-b"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-tui", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{"conversation-b"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-web", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{"conversation-a"}, ObservedAtMS: 200_500, Status: "idle"},
		{InstanceID: "client-app", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{"conversation-a"}, ObservedAtMS: 200_500, Status: "active"},
		{InstanceID: "client-mobile", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{"conversation-b"}, ObservedAtMS: 200_500, Status: "idle"},
	}
	sessionView, err := deviceplacement.ObserveClientInstanceSessionView(
		deviceplacement.ClientInstanceSessionViewRequest{Owner: owner, Instances: instances},
	)
	if err != nil {
		t.Fatalf("observe paired session view: %v", err)
	}
	resourceView := fixtureClientInstanceResourceView(owner)
	resourceView.Instances = append([]deviceplacement.ClientInstanceSessionViewInstance(nil), sessionView.Instances...)
	if err := resourceView.Validate(); err != nil {
		t.Fatalf("validate paired resource view: %v", err)
	}
	sessionSource := &fixtureClientInstanceSessionViewSource{value: sessionView}
	resourceSource := &fixtureClientInstanceResourceViewSource{value: resourceView}

	testRoutes := http.NewServeMux()
	testRoutes.Handle(clientInstanceSessionViewCandidatePath,
		newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{
			Enabled: true,
			Source:  sessionSource,
		}))
	testRoutes.Handle(clientInstanceResourceViewCandidatePath,
		newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
			Enabled: true,
			Source:  resourceSource,
		}))
	server := httptest.NewServer(authenticator.Handler(testRoutes))
	t.Cleanup(server.Close)

	client := server.Client()
	sessionResponse := snaplinkConversationRequest(t, client, server.URL, token,
		http.MethodGet, clientInstanceSessionViewCandidatePath, "", "")
	if sessionResponse.StatusCode != http.StatusOK {
		t.Fatalf("paired session view status=%d body=%q", sessionResponse.StatusCode, readConversationClientBody(t, sessionResponse))
	}
	var gotSession deviceplacement.ClientInstanceSessionViewObservation
	sessionBody := readConversationClientBody(t, sessionResponse)
	if err := json.Unmarshal([]byte(sessionBody), &gotSession); err != nil {
		t.Fatalf("decode paired session view: %v body=%q", err, sessionBody)
	}
	if err := gotSession.Validate(); err != nil {
		t.Fatalf("validate paired session view: %v", err)
	}

	resourceResponse := snaplinkConversationRequest(t, client, server.URL, token,
		http.MethodGet, clientInstanceResourceViewCandidatePath, "", "")
	if resourceResponse.StatusCode != http.StatusOK {
		t.Fatalf("paired resource view status=%d body=%q", resourceResponse.StatusCode, readConversationClientBody(t, resourceResponse))
	}
	var gotResource deviceplacement.ClientInstanceResourceViewObservation
	resourceBody := readConversationClientBody(t, resourceResponse)
	if err := json.Unmarshal([]byte(resourceBody), &gotResource); err != nil {
		t.Fatalf("decode paired resource view: %v body=%q", err, resourceBody)
	}
	if err := gotResource.Validate(); err != nil {
		t.Fatalf("validate paired resource view: %v", err)
	}

	if gotSession.Owner != owner || gotResource.Owner != owner ||
		sessionSource.calls != 1 || resourceSource.calls != 1 ||
		sessionSource.owner.Issuer != owner.Issuer || sessionSource.owner.Subject != owner.Subject || sessionSource.owner.TenantID != owner.TenantID ||
		resourceSource.owner.Issuer != owner.Issuer || resourceSource.owner.Subject != owner.Subject || resourceSource.owner.TenantID != owner.TenantID ||
		len(gotSession.Instances) != 5 || len(gotResource.Instances) != 5 ||
		gotSession.Authority != (deviceplacement.ClientInstanceSessionViewAuthority{}) ||
		gotResource.Authority != (deviceplacement.ClientInstanceResourceViewAuthority{}) {
		t.Fatalf("paired owner/source/authority projection session=%#v resource=%#v session_source=%#v resource_source=%#v", gotSession, gotResource, sessionSource, resourceSource)
	}
	wantInstanceIDs := []string{"client-app", "client-cli", "client-mobile", "client-tui", "client-web"}
	for index, wantID := range wantInstanceIDs {
		if gotSession.Instances[index].InstanceID != wantID || gotResource.Instances[index].InstanceID != wantID {
			t.Fatalf("paired instance order drifted at index %d: session=%#v resource=%#v", index, gotSession.Instances, gotResource.Instances)
		}
	}
	if string(marshalConvergenceJSON(t, gotSession.Instances)) != string(marshalConvergenceJSON(t, gotResource.Instances)) {
		t.Fatalf("paired instance rows drifted: session=%#v resource=%#v", gotSession.Instances, gotResource.Instances)
	}
	if len(gotResource.Devices) != 1 || gotResource.Devices[0].DeviceID != "device-a" ||
		gotResource.Devices[0].RunnerInstanceID != "runner-a" || !gotResource.ReadOnly {
		t.Fatalf("paired resource devices=%#v", gotResource.Devices)
	}

	// Conversation read access is intentionally insufficient for the resource
	// projection.  The same owner may still read the session metadata, while
	// the device observation route must fail at its separate device scope gate.
	underScopedToken := snaplinkForgeLoginWithPolicy(
		t, ssoClient, issuer, "forge-console",
		[]string{"openid", "profile", "forge:conversations:read"},
		[]string{snaplinkForgeTestAudience},
	)
	underScopedSession := snaplinkConversationRequest(t, client, server.URL, underScopedToken,
		http.MethodGet, clientInstanceSessionViewCandidatePath, "", "")
	if underScopedSession.StatusCode != http.StatusOK {
		t.Fatalf("under-scoped session view status=%d body=%q", underScopedSession.StatusCode, readConversationClientBody(t, underScopedSession))
	}
	_ = underScopedSession.Body.Close()
	underScopedResource := snaplinkConversationRequest(t, client, server.URL, underScopedToken,
		http.MethodGet, clientInstanceResourceViewCandidatePath, "", "")
	if underScopedResource.StatusCode != http.StatusForbidden {
		t.Fatalf("under-scoped resource view status=%d body=%q, want 403", underScopedResource.StatusCode, readConversationClientBody(t, underScopedResource))
	}
	_ = underScopedResource.Body.Close()
	if sessionSource.calls != 2 || resourceSource.calls != 1 {
		t.Fatalf("under-scoped projection source calls session=%d resource=%d; resource source must not run", sessionSource.calls, resourceSource.calls)
	}

	// The ordinary production constructor remains closed even for the
	// full-scope token.  The pair above is mounted only on the test mux.
	production := httptest.NewServer(authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil)))
	t.Cleanup(production.Close)
	for _, path := range []string{clientInstanceSessionViewCandidatePath, clientInstanceResourceViewCandidatePath} {
		response := snaplinkConversationRequest(t, production.Client(), production.URL, token,
			http.MethodGet, path, "", "")
		body := readConversationClientBody(t, response)
		if response.StatusCode != http.StatusNotFound || body != string(notFoundBody) {
			t.Fatalf("default production client-instance path=%s status=%d body=%q, want closed 404", path, response.StatusCode, body)
		}
	}
}

func marshalConvergenceJSON(t *testing.T, value any) []byte {
	t.Helper()
	encoded, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return encoded
}
