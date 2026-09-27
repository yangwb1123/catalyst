package appserver

// This test proves that the owner-scoped Conversation change stream survives
// the real Snaplink JWT boundary. It is intentionally read-only: the fixture
// exposes only dense change metadata and cannot create Prompts, Runs, devices,
// leases, or Runner work.

import (
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/authn"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestSnaplinkAuthenticatedConversationChangesStreamE2E(t *testing.T) {
	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)
	authenticator, err := authn.New(authn.Config{
		Issuer:              issuer,
		Audience:            snaplinkForgeTestAudience,
		JWKSURL:             issuer + "/.well-known/jwks.json",
		ExpectedTenantID:    snaplinkForgeTestTenant,
		ExpectedSubjectID:   snaplinkForgeTestUser,
		JWKSHTTPClient:      ssoClient,
		JWKSRefreshInterval: 24 * time.Hour,
	})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(authenticator.Close)

	backend := &fakeConversationBackend{changePage: model.OwnedConversationChangePage{
		AfterCursor: 0, ScannedThroughCursor: 1,
		Changes: []model.Change{{
			Cursor: 1, SchemaVersion: 1,
			ConversationID:   "snaplink-stream-conversation",
			EntityID:         "snaplink-stream-conversation",
			AggregateVersion: 1,
			Kind:             "conversation_created",
			CreatedAtMS:      1,
		}},
	}}
	server := httptest.NewServer(authenticator.Handler(newConversationRoutesWithBackend(backend)))
	t.Cleanup(server.Close)

	request, err := http.NewRequest(http.MethodGet,
		server.URL+conversationChangesStreamPath+"?after_cursor=0&limit=1&wait_ms=0", nil)
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Authorization", "Bearer "+token)
	request.Header.Set("Accept", "text/event-stream")
	response, err := server.Client().Do(request)
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	body, err := io.ReadAll(response.Body)
	if err != nil {
		t.Fatal(err)
	}
	if response.StatusCode != http.StatusOK ||
		!strings.HasPrefix(response.Header.Get("Content-Type"), "text/event-stream") {
		t.Fatalf("Snaplink stream status=%d content-type=%q body=%q", response.StatusCode, response.Header.Get("Content-Type"), body)
	}
	if !strings.Contains(string(body), "event: conversation_changes\n") ||
		!strings.Contains(string(body), "id: 1\n") ||
		!strings.Contains(string(body), `"conversation_id":"snaplink-stream-conversation"`) {
		t.Fatalf("Snaplink stream body=%q", body)
	}
	wantOwner := model.Owner{Issuer: issuer, Subject: snaplinkForgeTestUser, TenantID: snaplinkForgeTestTenant}
	if backend.changeOwner != wantOwner || backend.changeAfter != 0 || backend.changeLimit != 1 {
		t.Fatalf("Snaplink stream backend call owner=%#v after=%d limit=%d want owner=%#v after=0 limit=1", backend.changeOwner, backend.changeAfter, backend.changeLimit, wantOwner)
	}

	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	productionRequest, err := http.NewRequest(http.MethodGet,
		server.URL+conversationChangesStreamPath+"?after_cursor=0&wait_ms=0", nil)
	if err != nil {
		t.Fatal(err)
	}
	productionRequest.Header.Set("Authorization", "Bearer "+token)
	// The ordinary constructor has no Runtime backend in this fixture; preserve
	// the fail-closed unavailable response even for a fully scoped token.
	productionResponse := httptest.NewRecorder()
	production.ServeHTTP(productionResponse, productionRequest)
	if productionResponse.Code != http.StatusServiceUnavailable ||
		!strings.Contains(productionResponse.Body.String(), "conversation_service_unavailable") {
		t.Fatalf("production stream status=%d body=%q, want unavailable without a Runtime backend", productionResponse.Code, productionResponse.Body.String())
	}
}
