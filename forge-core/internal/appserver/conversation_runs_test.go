package appserver

import (
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
	"net/http"
	"strings"
	"testing"
)

func TestConversationRunRoutesUseVerifiedOwnerAndBoundedCursors(t *testing.T) {
	backend := &fakeConversationBackend{
		runPage: runmodel.OwnedRunPage{
			ConversationID: "conversation-1",
			Runs: []runmodel.OwnedRunSummary{{
				RunID: "run-2", PromptID: "prompt-2", CreatedAtMS: 20,
				LatestSequence: 3, Status: "nonterminal",
			}},
		},
		timelinePage: runmodel.OwnedRunTimelinePage{
			ConversationID: "conversation-1", RunID: "run-2", AfterSequence: 2,
			ScannedThroughSequence: 3,
			Events:                 []runmodel.OwnedRunEventSummary{{Sequence: 3, EmittedAtMS: 30, Type: "turn_started"}},
		},
	}
	identity, handler := conversationTestHandler(t, backend)
	owner := model.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	assertConversationRunPageRoute(t, handler, identity, backend, owner)
	assertConversationRunTimelineRoute(t, handler, identity, backend, owner)
	assertConversationRunRouteRejections(t, handler, identity, backend)
}

func assertConversationRunPageRoute(
	t *testing.T,
	handler http.Handler,
	identity *conversationTestIdentity,
	backend *fakeConversationBackend,
	owner model.Owner,
) {
	t.Helper()
	pageResponse := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationCollectionPath+"/conversation-1/runs?limit=5&before_created_at_ms=21&before_run_id=run-3",
		"forge:conversations:read", "", "", "")
	if pageResponse.Code != http.StatusOK || backend.runCalls != 1 || backend.runOwner != owner ||
		backend.runID != "conversation-1" || backend.runLimit != 5 || backend.runCursor == nil ||
		backend.runCursor.CreatedAtMS != 21 || backend.runCursor.RunID != "run-3" ||
		pageResponse.Body.String() == "" {
		t.Fatalf("run page status=%d body=%q backend=%#v", pageResponse.Code, pageResponse.Body.String(), backend)
	}
	if got := pageResponse.Body.String(); containsAny(got, "execution_json", "project_id", "tool_started", "answer") {
		t.Fatalf("run page contains fields outside the summary allowlist: %q", got)
	}
	defaultLimitResponse := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationCollectionPath+"/conversation-1/runs", "forge:conversations:read", "", "", "")
	if defaultLimitResponse.Code != http.StatusOK || backend.runCalls != 2 || backend.runLimit != 25 || backend.runCursor != nil {
		t.Fatalf("default Run page status=%d backend=%#v", defaultLimitResponse.Code, backend)
	}
}

func assertConversationRunTimelineRoute(
	t *testing.T,
	handler http.Handler,
	identity *conversationTestIdentity,
	backend *fakeConversationBackend,
	owner model.Owner,
) {
	t.Helper()
	timelineResponse := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationCollectionPath+"/conversation-1/runs/run-2/timeline?after_sequence=2&limit=8",
		"forge:conversations:read", "", "", "")
	if timelineResponse.Code != http.StatusOK || backend.timelineCalls != 1 || backend.timelineOwner != owner ||
		backend.timelineConvID != "conversation-1" || backend.timelineRunID != "run-2" ||
		backend.timelineAfter != 2 || backend.timelineLimit != 8 {
		t.Fatalf("Run timeline status=%d body=%q backend=%#v", timelineResponse.Code, timelineResponse.Body.String(), backend)
	}
	if got := timelineResponse.Body.String(); containsAny(got, "assistant_delta", "output", "message", "path") {
		t.Fatalf("Run timeline contains event payload: %q", got)
	}
}

func assertConversationRunRouteRejections(
	t *testing.T,
	handler http.Handler,
	identity *conversationTestIdentity,
	backend *fakeConversationBackend,
) {
	t.Helper()
	noScope := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationCollectionPath+"/conversation-1/runs", "", "", "", "")
	if noScope.Code != http.StatusForbidden || backend.runCalls != 2 {
		t.Fatalf("Run list without read scope status=%d calls=%d", noScope.Code, backend.runCalls)
	}
	invalidCursor := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationCollectionPath+"/conversation-1/runs?before_created_at_ms=21", "forge:conversations:read", "", "", "")
	if invalidCursor.Code != http.StatusBadRequest || backend.runCalls != 2 {
		t.Fatalf("partial Run cursor status=%d calls=%d", invalidCursor.Code, backend.runCalls)
	}
	tooMany := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationCollectionPath+"/conversation-1/runs?limit=26", "forge:conversations:read", "", "", "")
	if tooMany.Code != http.StatusBadRequest || backend.runCalls != 2 {
		t.Fatalf("over-limit Run page status=%d calls=%d", tooMany.Code, backend.runCalls)
	}
	unknown := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationCollectionPath+"/conversation-1/runs?project_path=/tmp", "forge:conversations:read", "", "", "")
	if unknown.Code != http.StatusBadRequest || backend.runCalls != 2 {
		t.Fatalf("unknown Run query status=%d calls=%d", unknown.Code, backend.runCalls)
	}
}

func containsAny(value string, fragments ...string) bool {
	for _, fragment := range fragments {
		if strings.Contains(value, fragment) {
			return true
		}
	}
	return false
}
