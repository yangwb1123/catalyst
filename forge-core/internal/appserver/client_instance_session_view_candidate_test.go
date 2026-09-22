package appserver

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestClientInstanceSessionViewCandidateDefaultsClosed(t *testing.T) {
	source := &fixtureClientInstanceSessionViewSource{}
	for _, config := range []*clientInstanceSessionViewCandidateConfig{
		nil,
		{},
		{Source: source},
	} {
		request := httptest.NewRequest(http.MethodGet, clientInstanceSessionViewCandidatePath, nil)
		response := httptest.NewRecorder()
		newClientInstanceSessionViewCandidateRoutes(config).ServeHTTP(response, request)
		if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
			t.Fatalf("default-disabled session view response=%d body=%q", response.Code, response.Body.String())
		}
	}
	if source.calls != 0 {
		t.Fatalf("default-disabled candidate invoked source %d times", source.calls)
	}
}

func TestClientInstanceSessionViewCandidateUsesVerifiedOwnerAndFixtureSource(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	source := &fixtureClientInstanceSessionViewSource{value: fixtureClientInstanceSessionView(owner)}
	handler := authenticator.Handler(newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{
		Enabled: true,
		Source:  source,
	}))
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		clientInstanceSessionViewCandidatePath, clientInstanceSessionViewCandidateScope, "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("session view candidate status=%d body=%q", response.Code, response.Body.String())
	}
	var got deviceplacement.ClientInstanceSessionViewObservation
	if err := json.Unmarshal(response.Body.Bytes(), &got); err != nil {
		t.Fatalf("decode session view candidate: %v body=%q", err, response.Body.String())
	}
	if err := got.Validate(); err != nil {
		t.Fatalf("validate session view candidate: %v", err)
	}
	if source.calls != 1 || source.owner != (model.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}) ||
		got.Owner != owner || len(got.Instances) != 2 || got.Instances[0].InstanceID != "client-cli" ||
		got.Instances[1].InstanceID != "client-web" || got.Authority != (deviceplacement.ClientInstanceSessionViewAuthority{}) ||
		!got.ReadOnly {
		t.Fatalf("source=%#v response=%#v", source, got)
	}
	if !strings.Contains(response.Body.String(), `"owner_declaration_unverified":true`) {
		t.Fatalf("candidate omitted unverified marker: %q", response.Body.String())
	}

	production := authenticator.Handler(newConversationRoutes(nil))
	request := httptest.NewRequest(http.MethodGet, clientInstanceSessionViewCandidatePath, nil)
	request.Header.Set("Authorization", "Bearer "+identity.token(clientInstanceSessionViewCandidateScope))
	productionResponse := httptest.NewRecorder()
	production.ServeHTTP(productionResponse, request)
	if productionResponse.Code != http.StatusNotFound {
		t.Fatalf("production session view route status=%d body=%q", productionResponse.Code, productionResponse.Body.String())
	}
}

func TestClientInstanceSessionViewCandidateRejectsQueryBodyScopeAndForeignOwner(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	source := &fixtureClientInstanceSessionViewSource{value: fixtureClientInstanceSessionView(owner)}
	handler := authenticator.Handler(newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{
		Enabled: true,
		Source:  source,
	}))
	query := requestConversationAPI(t, handler, identity, http.MethodGet,
		clientInstanceSessionViewCandidatePath+"?limit=1", clientInstanceSessionViewCandidateScope, "", "", "")
	if query.Code != http.StatusBadRequest || source.calls != 0 {
		t.Fatalf("query status=%d calls=%d body=%q", query.Code, source.calls, query.Body.String())
	}
	method := requestConversationAPI(t, handler, identity, http.MethodPost,
		clientInstanceSessionViewCandidatePath, clientInstanceSessionViewCandidateScope, "application/json", "", `{}`)
	if method.Code != http.StatusMethodNotAllowed || source.calls != 0 {
		t.Fatalf("method status=%d calls=%d body=%q", method.Code, source.calls, method.Body.String())
	}
	noScope := requestConversationAPI(t, handler, identity, http.MethodGet,
		clientInstanceSessionViewCandidatePath, "forge:conversations:write", "", "", "")
	if noScope.Code != http.StatusForbidden || source.calls != 0 {
		t.Fatalf("scope status=%d calls=%d body=%q", noScope.Code, source.calls, noScope.Body.String())
	}
	foreign := &fixtureClientInstanceSessionViewSource{value: fixtureClientInstanceSessionView(deviceplacement.Owner{
		Issuer: identity.issuer, Subject: "account-foreign", TenantID: "tenant-slate",
	})}
	foreignHandler := authenticator.Handler(newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{
		Enabled: true,
		Source:  foreign,
	}))
	foreignResponse := requestConversationAPI(t, foreignHandler, identity, http.MethodGet,
		clientInstanceSessionViewCandidatePath, clientInstanceSessionViewCandidateScope, "", "", "")
	if foreignResponse.Code != http.StatusBadGateway ||
		!strings.Contains(foreignResponse.Body.String(), `"code":"client_instance_session_view_invalid"`) {
		t.Fatalf("foreign source status=%d body=%q", foreignResponse.Code, foreignResponse.Body.String())
	}
}

func TestClientInstanceSessionViewCandidateSourceFailureFailsClosed(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	source := &fixtureClientInstanceSessionViewSource{err: errors.New("fixture unavailable")}
	handler := authenticator.Handler(newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{
		Enabled: true,
		Source:  source,
	}))
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		clientInstanceSessionViewCandidatePath, clientInstanceSessionViewCandidateScope, "", "", "")
	if response.Code != http.StatusBadGateway ||
		!strings.Contains(response.Body.String(), `"code":"conversation_service_error"`) {
		t.Fatalf("source failure status=%d body=%q", response.Code, response.Body.String())
	}
}

type fixtureClientInstanceSessionViewSource struct {
	value deviceplacement.ClientInstanceSessionViewObservation
	err   error
	calls int
	owner model.Owner
}

func (source *fixtureClientInstanceSessionViewSource) ReadOwnedClientInstanceSessionView(
	_ context.Context,
	owner model.Owner,
) (deviceplacement.ClientInstanceSessionViewObservation, error) {
	source.calls++
	source.owner = owner
	if source.err != nil {
		return deviceplacement.ClientInstanceSessionViewObservation{}, source.err
	}
	return source.value, nil
}

func fixtureClientInstanceSessionView(owner deviceplacement.Owner) deviceplacement.ClientInstanceSessionViewObservation {
	view, err := deviceplacement.ObserveClientInstanceSessionView(deviceplacement.ClientInstanceSessionViewRequest{
		Owner: owner,
		Instances: []deviceplacement.ClientInstanceSessionViewInstance{
			{InstanceID: "client-web", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{"conversation-a"}, ObservedAtMS: 200_500, Status: "idle"},
			{InstanceID: "client-cli", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{"conversation-a", "conversation-b"}, ObservedAtMS: 200_500, Status: "active"},
		},
	})
	if err != nil {
		panic(err)
	}
	return view
}
