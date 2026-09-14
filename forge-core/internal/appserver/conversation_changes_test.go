package appserver

import (
	"encoding/json"
	"fmt"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"net/http"
	"strings"
	"testing"
)

func TestConversationChangesRouteForwardsVerifiedOwnerAndDensePage(t *testing.T) {
	backend := &fakeConversationBackend{changePage: model.OwnedConversationChangePage{
		AfterCursor: 4, ScannedThroughCursor: 5, HasMore: true,
		Changes: []model.Change{{
			Cursor: 5, SchemaVersion: 1, ConversationID: "c-owned", EntityID: "p-owned",
			AggregateVersion: 2, Kind: "prompt_appended", CreatedAtMS: 17,
		}},
	}}
	identity, handler := conversationTestHandler(t, backend)
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationChangesPath+"?after_cursor=4&limit=1", "forge:conversations:read", "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("GET status=%d body=%q", response.Code, response.Body.String())
	}
	wantOwner := model.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	if backend.changeCalls != 1 || backend.changeOwner != wantOwner || backend.changeAfter != 4 || backend.changeLimit != 1 {
		t.Fatalf("replay call = %#v", backend)
	}
	var root map[string]json.RawMessage
	if err := json.Unmarshal(response.Body.Bytes(), &root); err != nil ||
		!exactJSONKeys(root, "after_cursor", "scanned_through_cursor", "has_more", "changes") {
		t.Fatalf("replay response contains an unexpected shape: %s, %v", response.Body.String(), err)
	}
	if !strings.Contains(response.Body.String(), `"cursor":5`) ||
		!strings.Contains(response.Body.String(), `"scanned_through_cursor":5`) {
		t.Fatalf("dense replay response = %q", response.Body.String())
	}
	var changes []map[string]json.RawMessage
	if err := json.Unmarshal(root["changes"], &changes); err != nil || len(changes) != 1 ||
		!exactJSONKeys(changes[0], "cursor", "schema_version", "conversation_id", "entity_id",
			"aggregate_version", "kind", "created_at_ms") {
		t.Fatalf("change projection contains unexpected fields: %s, %v", root["changes"], err)
	}
}

func TestConversationChangesRouteRejectsInvalidQueriesAndBodies(t *testing.T) {
	backend := &fakeConversationBackend{}
	identity, handler := conversationTestHandler(t, backend)
	targets := []string{
		conversationChangesPath,
		conversationChangesPath + "?after_cursor=",
		conversationChangesPath + "?after_cursor=-1",
		conversationChangesPath + "?after_cursor=+1",
		conversationChangesPath + "?after_cursor=9223372036854775808",
		conversationChangesPath + "?after_cursor=1&after_cursor=2",
		conversationChangesPath + "?after_cursor=1&limit=1&limit=2",
		conversationChangesPath + "?after_cursor=1&limit=0",
		conversationChangesPath + "?after_cursor=1&limit=129",
		conversationChangesPath + "?after_cursor=1&limit=1&unknown=x",
	}
	for _, target := range targets {
		response := requestConversationAPI(t, handler, identity, http.MethodGet, target,
			"forge:conversations:read", "", "", "")
		if response.Code != http.StatusBadRequest {
			t.Errorf("query %q status=%d body=%q", target, response.Code, response.Body.String())
		}
	}
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationChangesPath+"?after_cursor=1", "forge:conversations:read", "", "", "body")
	if response.Code != http.StatusBadRequest {
		t.Fatalf("GET with body status=%d body=%q", response.Code, response.Body.String())
	}
	if backend.changeCalls != 0 {
		t.Fatalf("invalid replay request reached backend %d times", backend.changeCalls)
	}
}

func TestConversationChangesRouteRequiresReadScope(t *testing.T) {
	backend := &fakeConversationBackend{}
	identity, handler := conversationTestHandler(t, backend)
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationChangesPath+"?after_cursor=0", "forge:conversations:write", "", "", "")
	if response.Code != http.StatusForbidden || backend.changeCalls != 0 {
		t.Fatalf("missing read scope status=%d calls=%d body=%q", response.Code, backend.changeCalls, response.Body.String())
	}
}

func TestConversationChangesRouteIsMountedByApplicationRouter(t *testing.T) {
	backend := &fakeConversationBackend{changePage: model.OwnedConversationChangePage{
		AfterCursor: 0, ScannedThroughCursor: 0, Changes: []model.Change{},
	}}
	identity, sessions := conversationTestHandler(t, backend)
	handler, err := newRoutesWithSessions(
		BuildInfo{Version: "v1", Commit: "abc123"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationChangesPath+"?after_cursor=0", "forge:conversations:read", "", "", "")
	if response.Code != http.StatusOK || backend.changeCalls != 1 || backend.changeLimit != conversationPageDefault {
		t.Fatalf("mounted replay route status=%d calls=%d body=%q", response.Code, backend.changeCalls, response.Body.String())
	}
}

func TestConversationChangesRouteRejectsCursorOverflow(t *testing.T) {
	backend := &fakeConversationBackend{}
	identity, handler := conversationTestHandler(t, backend)
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationChangesPath+"?after_cursor="+fmt.Sprint(maxSQLiteCursor+1), "forge:conversations:read", "", "", "")
	if response.Code != http.StatusBadRequest || backend.changeCalls != 0 {
		t.Fatalf("cursor overflow status=%d calls=%d body=%q", response.Code, backend.changeCalls, response.Body.String())
	}
}
