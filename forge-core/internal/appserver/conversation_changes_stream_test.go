package appserver

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestConversationChangesStreamRouteWritesOwnerBoundSSEPage(t *testing.T) {
	backend := &fakeConversationBackend{changePage: model.OwnedConversationChangePage{
		AfterCursor: 4, ScannedThroughCursor: 5,
		Changes: []model.Change{{
			Cursor: 5, SchemaVersion: 1, ConversationID: "conversation-1", EntityID: "prompt-1",
			AggregateVersion: 2, Kind: "prompt_appended", CreatedAtMS: 17,
		}},
	}}
	identity, handler := conversationTestHandler(t, backend)
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationChangesStreamPath+"?after_cursor=4&limit=1", "forge:conversations:read", "", "", "")
	if response.Code != http.StatusOK || !strings.HasPrefix(response.Header().Get("Content-Type"), "text/event-stream") {
		t.Fatalf("stream status=%d content-type=%q body=%q", response.Code, response.Header().Get("Content-Type"), response.Body.String())
	}
	if backend.changeCalls != 1 || backend.changeAfter != 4 || backend.changeLimit != 1 {
		t.Fatalf("stream backend call=%#v", backend)
	}
	wantOwner := model.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	if backend.changeOwner != wantOwner {
		t.Fatalf("stream owner=%#v want=%#v", backend.changeOwner, wantOwner)
	}
	body := response.Body.String()
	if !strings.Contains(body, "event: conversation_changes\n") ||
		!strings.Contains(body, "id: 5\n") ||
		!strings.Contains(body, `"conversation_id":"conversation-1"`) {
		t.Fatalf("unexpected SSE body=%q", body)
	}
	dataStart := strings.Index(body, "data: ")
	if dataStart < 0 {
		t.Fatalf("stream omitted data frame: %q", body)
	}
	dataLine := strings.Split(body[dataStart:], "\n\n")[0]
	dataLine = strings.TrimPrefix(dataLine, "data: ")
	var page model.OwnedConversationChangePage
	if err := json.Unmarshal([]byte(dataLine), &page); err != nil {
		t.Fatalf("decode SSE data: %v body=%q", err, body)
	}
	if page.AfterCursor != 4 || page.ScannedThroughCursor != 5 || len(page.Changes) != 1 {
		t.Fatalf("decoded SSE page=%#v", page)
	}
	if !strings.Contains(body, `"scanned_through_cursor":5`) {
		t.Fatalf("stream omitted scanned cursor: %q", body)
	}
}

func TestConversationChangesStreamRouteReturnsNoContentWhenNoChangeIsReady(t *testing.T) {
	backend := &fakeConversationBackend{changePage: model.OwnedConversationChangePage{
		AfterCursor: 7, ScannedThroughCursor: 7, Changes: []model.Change{},
	}}
	identity, handler := conversationTestHandler(t, backend)
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationChangesStreamPath+"?after_cursor=7&wait_ms=0", "forge:conversations:read", "", "", "")
	if response.Code != http.StatusNoContent || response.Body.Len() != 0 || backend.changeCalls != 1 {
		t.Fatalf("empty stream status=%d calls=%d body=%q", response.Code, backend.changeCalls, response.Body.String())
	}
}

func TestConversationChangesStreamRouteWakesOnLaterOwnerChange(t *testing.T) {
	page := model.OwnedConversationChangePage{
		AfterCursor: 0, ScannedThroughCursor: 1,
		Changes: []model.Change{{
			Cursor: 1, SchemaVersion: 1, ConversationID: "conversation-1", EntityID: "conversation-1",
			AggregateVersion: 1, Kind: "conversation_created", CreatedAtMS: 17,
		}},
	}
	backend := &streamSequenceConversationBackend{
		fakeConversationBackend: &fakeConversationBackend{},
		pages: []model.OwnedConversationChangePage{
			{AfterCursor: 0, ScannedThroughCursor: 0, Changes: []model.Change{}},
			page,
		},
	}
	identity, handler := conversationTestHandler(t, backend)
	server := httptest.NewServer(handler)
	t.Cleanup(server.Close)
	request, err := http.NewRequest(http.MethodGet,
		server.URL+conversationChangesStreamPath+"?after_cursor=0&limit=1&wait_ms=500", nil)
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Authorization", "Bearer "+identity.token("forge:conversations:read"))
	response, err := server.Client().Do(request)
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	body, err := io.ReadAll(response.Body)
	if err != nil {
		t.Fatal(err)
	}
	if response.StatusCode != http.StatusOK || !strings.Contains(string(body), "id: 1\n") || backend.calls != 2 {
		t.Fatalf("wake stream status=%d calls=%d body=%q", response.StatusCode, backend.calls, body)
	}
}

func TestConversationChangesStreamRouteRejectsInvalidQueriesBodiesAndScope(t *testing.T) {
	backend := &fakeConversationBackend{}
	identity, handler := conversationTestHandler(t, backend)
	for _, target := range []string{
		conversationChangesStreamPath,
		conversationChangesStreamPath + "?after_cursor=0&wait_ms=30001",
		conversationChangesStreamPath + "?after_cursor=0&wait_ms=-1",
		conversationChangesStreamPath + "?after_cursor=0&wait_ms=1&wait_ms=2",
		conversationChangesStreamPath + "?after_cursor=0&unknown=1",
	} {
		response := requestConversationAPI(t, handler, identity, http.MethodGet, target,
			"forge:conversations:read", "", "", "")
		if response.Code != http.StatusBadRequest {
			t.Errorf("query %q status=%d body=%q", target, response.Code, response.Body.String())
		}
	}
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationChangesStreamPath+"?after_cursor=0", "forge:conversations:read", "", "", "body")
	if response.Code != http.StatusBadRequest {
		t.Fatalf("stream body status=%d body=%q", response.Code, response.Body.String())
	}
	response = requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationChangesStreamPath+"?after_cursor=0", "forge:conversations:write", "", "", "")
	if response.Code != http.StatusForbidden || backend.changeCalls != 0 {
		t.Fatalf("stream scope status=%d calls=%d body=%q", response.Code, backend.changeCalls, response.Body.String())
	}
}

func TestConversationChangesStreamRouteMapsBackendFailure(t *testing.T) {
	backend := &fakeConversationBackend{err: errStreamBackendFailure{}}
	identity, handler := conversationTestHandler(t, backend)
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationChangesStreamPath+"?after_cursor=0", "forge:conversations:read", "", "", "")
	if response.Code != http.StatusBadGateway || backend.changeCalls != 1 {
		t.Fatalf("stream backend failure status=%d calls=%d body=%q", response.Code, backend.changeCalls, response.Body.String())
	}
}

func TestConversationChangesStreamRouteIsMountedByApplicationRouter(t *testing.T) {
	backend := &fakeConversationBackend{changePage: model.OwnedConversationChangePage{
		AfterCursor: 0, ScannedThroughCursor: 1,
		Changes: []model.Change{{
			Cursor: 1, SchemaVersion: 1, ConversationID: "conversation-1", EntityID: "conversation-1",
			AggregateVersion: 1, Kind: "conversation_created", CreatedAtMS: 17,
		}},
	}}
	identity, sessions := conversationTestHandler(t, backend)
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "v1", Commit: "abc123"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	response := requestConversationAPI(t, routes, identity, http.MethodGet,
		conversationChangesStreamPath+"?after_cursor=0&limit=1&wait_ms=0", "forge:conversations:read", "", "", "")
	if response.Code != http.StatusOK || !strings.Contains(response.Body.String(), "event: conversation_changes") || backend.changeCalls != 1 {
		t.Fatalf("mounted stream status=%d calls=%d body=%q", response.Code, backend.changeCalls, response.Body.String())
	}
}

type streamSequenceConversationBackend struct {
	*fakeConversationBackend
	pages []model.OwnedConversationChangePage
	calls int
}

func (backend *streamSequenceConversationBackend) OwnedConversationChangesAfter(
	_ context.Context,
	owner model.Owner,
	after uint64,
	limit int,
) (model.OwnedConversationChangePage, error) {
	backend.calls++
	backend.changeOwner, backend.changeAfter, backend.changeLimit = owner, after, limit
	if backend.calls > len(backend.pages) {
		return backend.pages[len(backend.pages)-1], nil
	}
	return backend.pages[backend.calls-1], nil
}

type errStreamBackendFailure struct{}

func (errStreamBackendFailure) Error() string { return "stream backend failed" }
