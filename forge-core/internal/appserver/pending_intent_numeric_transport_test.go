package appserver

import (
	"context"
	"errors"
	consentmodel "forgeos/forge-core/internal/runtimebridge/consentmodel"
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"net/http"
	"strings"
	"testing"

	"forgeos/forge-core/internal/executionprofile"
)

type pendingIntentHTTPBackend struct {
	fakeConversationBackend
	submission *intentmodel.PendingRunIntentSubmissionResult
	page       *intentmodel.OwnedPendingRunIntentPage
	timeline   *intentmodel.OwnedPendingRunIntentTimelinePage
}

func (pendingIntentHTTPBackend) GrantProjectExecutionConsent(
	context.Context, model.Owner, string, string, [32]byte, uint64, string,
) (consentmodel.ProjectExecutionConsentGrantResult, error) {
	return consentmodel.ProjectExecutionConsentGrantResult{}, errors.New("test backend")
}

func (pendingIntentHTTPBackend) RevokeProjectExecutionConsent(
	context.Context, model.Owner, string, string,
) (consentmodel.ProjectExecutionConsentRevocationResult, error) {
	return consentmodel.ProjectExecutionConsentRevocationResult{}, errors.New("test backend")
}

func (backend pendingIntentHTTPBackend) SubmitOwnedPromptRunIntent(
	context.Context, model.Owner, string, string, string, uint64, intentmodel.ServerExecutionProfile,
) (intentmodel.PendingRunIntentSubmissionResult, error) {
	if backend.submission != nil {
		return *backend.submission, nil
	}
	return intentmodel.PendingRunIntentSubmissionResult{}, errors.New("test backend")
}

func (backend pendingIntentHTTPBackend) OwnedConversationPendingRunIntents(
	context.Context, model.Owner, string, *intentmodel.PendingRunIntentCursor, int,
) (intentmodel.OwnedPendingRunIntentPage, error) {
	if backend.page != nil {
		return *backend.page, nil
	}
	return intentmodel.OwnedPendingRunIntentPage{}, errors.New("test backend")
}

func (backend pendingIntentHTTPBackend) OwnedConversationPendingRunIntentTimeline(
	context.Context, model.Owner, string, string, uint64, int,
) (intentmodel.OwnedPendingRunIntentTimelinePage, error) {
	if backend.timeline != nil {
		return *backend.timeline, nil
	}
	return intentmodel.OwnedPendingRunIntentTimelinePage{}, errors.New("test backend")
}

func TestPendingIntentHTTPUsesJSONSafeNumericBoundaries(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	backend := &pendingIntentHTTPBackend{}
	handler := authenticator.Handler(newConversationRoutesWithInertExecutionAPI(backend, nil))
	conversationPath := conversationCollectionPath + "/conversation-1/run-intents"
	const maximum = "9007199254740991"

	safePage := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationPath+"?before_submitted_at_ms="+maximum+"&before_intent_id=intent-1",
		"forge:conversations:read", "", "", "")
	if safePage.Code == http.StatusBadRequest {
		t.Fatalf("JSON-safe pending intent cursor was rejected: status=%d body=%q", safePage.Code, safePage.Body.String())
	}
	unsafePage := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationPath+"?before_submitted_at_ms=9007199254740992&before_intent_id=intent-1",
		"forge:conversations:read", "", "", "")
	if unsafePage.Code != http.StatusBadRequest {
		t.Fatalf("unsafe pending intent cursor status=%d body=%q", unsafePage.Code, unsafePage.Body.String())
	}

	intentIDPath := conversationPath + "/intent-1/timeline"
	safeTimeline := requestConversationAPI(t, handler, identity, http.MethodGet,
		intentIDPath+"?after_sequence="+maximum,
		"forge:conversations:read", "", "", "")
	if safeTimeline.Code == http.StatusBadRequest {
		t.Fatalf("JSON-safe pending intent sequence was rejected: status=%d body=%q", safeTimeline.Code, safeTimeline.Body.String())
	}
	unsafeTimeline := requestConversationAPI(t, handler, identity, http.MethodGet,
		intentIDPath+"?after_sequence=9007199254740992",
		"forge:conversations:read", "", "", "")
	if unsafeTimeline.Code != http.StatusBadRequest {
		t.Fatalf("unsafe pending intent sequence status=%d body=%q", unsafeTimeline.Code, unsafeTimeline.Body.String())
	}

	safeSubmit := requestConversationAPI(t, handler, identity, http.MethodPost, conversationPath,
		"forge:conversations:write", "application/json", "pending-safe", `{"content":"prompt","expected_version":9007199254740991}`)
	if safeSubmit.Code == http.StatusBadRequest {
		t.Fatalf("JSON-safe pending intent version was rejected: status=%d body=%q", safeSubmit.Code, safeSubmit.Body.String())
	}
	unsafeSubmit := requestConversationAPI(t, handler, identity, http.MethodPost, conversationPath,
		"forge:conversations:write", "application/json", "pending-unsafe", `{"content":"prompt","expected_version":9007199254740992}`)
	if unsafeSubmit.Code != http.StatusBadRequest {
		t.Fatalf("unsafe pending intent version status=%d body=%q", unsafeSubmit.Code, unsafeSubmit.Body.String())
	}
}

func TestPendingIntentHTTPRejectsUnsafeBackendResponses(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	const maximum = uint64(9_007_199_254_740_991)
	unsafePage := &intentmodel.OwnedPendingRunIntentPage{
		ConversationID: "conversation-1",
		Intents: []intentmodel.PendingRunIntent{{
			IntentID: "intent-1", ConversationID: "conversation-1", PromptID: "prompt-1",
			ProjectID: "project-1", ProfileID: "profile-1", SubmittedAtMS: maximum,
			AggregateVersion: maximum + 1, LatestSequence: 1, Status: "pending",
		}},
	}
	pageHandler := authenticator.Handler(newConversationRoutesWithInertExecutionAPI(
		&pendingIntentHTTPBackend{page: unsafePage}, nil,
	))
	pageResponse := requestConversationAPI(t, pageHandler, identity, http.MethodGet,
		conversationCollectionPath+"/conversation-1/run-intents",
		"forge:conversations:read", "", "", "")
	if pageResponse.Code != http.StatusBadGateway || !strings.Contains(pageResponse.Body.String(), "conversation_service_error") {
		t.Fatalf("unsafe pending intent page status=%d body=%q", pageResponse.Code, pageResponse.Body.String())
	}

	unsafeTimeline := &intentmodel.OwnedPendingRunIntentTimelinePage{
		ConversationID: "conversation-1", IntentID: "intent-1", AfterSequence: 0,
		ScannedThroughSequence: maximum,
		Events: []intentmodel.PendingRunIntentEventSummary{{
			EventID: "event-1", Sequence: maximum + 1, EmittedAtMS: maximum, Type: "submitted",
		}},
	}
	timelineHandler := authenticator.Handler(newConversationRoutesWithInertExecutionAPI(
		&pendingIntentHTTPBackend{timeline: unsafeTimeline}, nil,
	))
	timelineResponse := requestConversationAPI(t, timelineHandler, identity, http.MethodGet,
		conversationCollectionPath+"/conversation-1/run-intents/intent-1/timeline",
		"forge:conversations:read", "", "", "")
	if timelineResponse.Code != http.StatusBadGateway || !strings.Contains(timelineResponse.Body.String(), "conversation_service_error") {
		t.Fatalf("unsafe pending intent timeline status=%d body=%q", timelineResponse.Code, timelineResponse.Body.String())
	}

	var digest [32]byte
	profiles, err := executionprofile.New([]executionprofile.Binding{{
		ProjectID: "project-1",
		Profile:   intentmodel.ServerExecutionProfile{ID: "profile-1", SHA256: digest},
	}})
	if err != nil {
		t.Fatal(err)
	}
	unsafeSubmission := &intentmodel.PendingRunIntentSubmissionResult{
		Prompt: model.ConversationPrompt{
			ID: "prompt-1", ConversationID: "conversation-1", Role: "user", Content: "prompt",
			CreatedAtMS: maximum + 1,
		},
		Intent: intentmodel.PendingRunIntent{
			IntentID: "intent-1", ConversationID: "conversation-1", PromptID: "prompt-1",
			ProjectID: "project-1", ProfileID: "profile-1", SubmittedAtMS: maximum,
			AggregateVersion: maximum, LatestSequence: 1, Status: "pending",
		},
		InitialEvent: intentmodel.PendingRunIntentEventSummary{
			EventID: "event-1", Sequence: 1, EmittedAtMS: maximum, Type: "submitted",
		},
	}
	submissionBackend := &pendingIntentHTTPBackend{
		fakeConversationBackend: fakeConversationBackend{projectIdentity: model.OwnedProjectConversationIdentity{
			ConversationID: "conversation-1", ProjectID: "project-1",
		}},
		submission: unsafeSubmission,
	}
	submissionHandler := authenticator.Handler(newConversationRoutesWithInertExecutionAPI(submissionBackend, profiles))
	submissionResponse := requestConversationAPI(t, submissionHandler, identity, http.MethodPost,
		conversationCollectionPath+"/conversation-1/run-intents",
		"forge:conversations:write", "application/json", "pending-unsafe-response",
		`{"content":"prompt","expected_version":1}`)
	if submissionResponse.Code != http.StatusBadGateway || !strings.Contains(submissionResponse.Body.String(), "conversation_service_error") {
		t.Fatalf("unsafe pending intent submission status=%d body=%q", submissionResponse.Code, submissionResponse.Body.String())
	}
}
