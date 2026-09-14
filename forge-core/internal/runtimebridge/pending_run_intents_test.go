package runtimebridge

import (
	"context"
	"encoding/json"
	"fmt"
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"path/filepath"
	"strings"
	"testing"
)

func TestSubmitOwnedPromptRunIntentPreservesOriginalReplayReceipt(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "app-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"}
	profile := intentmodel.ServerExecutionProfile{ID: "profile-current"}
	for index := range profile.SHA256 {
		profile.SHA256[index] = byte(index)
	}
	digest, err := json.Marshal(profile.SHA256)
	if err != nil {
		t.Fatal(err)
	}
	result := `{"prompt":{"id":"prompt-original","conversation_id":"conversation-1","role":"user","content":"compute this","created_at_ms":20},"intent":{"intent_id":"intent-original","conversation_id":"conversation-1","prompt_id":"prompt-original","project_id":"project-original","profile_id":"profile-original","submitted_at_ms":20,"aggregate_version":8,"latest_sequence":1,"status":"pending"},"initial_event":{"event_id":"event-original","seq":1,"emitted_at_ms":20,"type":"submitted"},"replayed":true}`
	writeFake(t, executable, responseScriptCheckingRequest(database, result, writeProtocolVersion,
		`"api_version":"forgeos.runtime-bridge/v2"`, `"operation":"submit_owned_prompt_run_intent"`,
		`"conversation_id":"conversation-1"`, `"content":"compute this"`, `"idempotency_key":"request-key"`,
		`"expected_version":99`, `"profile_id":"profile-current"`, `"profile_sha256":`+string(digest),
		`"owner":{"issuer":"https://identity.example","subject":"account-42","tenant_id":"tenant-slate"}`))
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	receipt, err := client.SubmitOwnedPromptRunIntent(context.Background(), owner,
		"conversation-1", "compute this", "request-key", 99, profile)
	if err != nil {
		t.Fatal(err)
	}
	if !receipt.Replayed || receipt.Intent.ProfileID != "profile-original" ||
		receipt.Intent.AggregateVersion != 8 || receipt.Prompt.ID != "prompt-original" ||
		receipt.InitialEvent.EventID != "event-original" {
		t.Fatalf("replay did not preserve original receipt: %#v", receipt)
	}
}

func TestOwnedConversationPendingRunIntentsUsesExclusiveOwnerCursor(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "app-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"}
	result := `{"conversation_id":"conversation-1","intents":[{"intent_id":"intent-y","conversation_id":"conversation-1","prompt_id":"prompt-y","project_id":"project-1","profile_id":"profile-1","submitted_at_ms":30,"aggregate_version":9,"latest_sequence":1,"status":"pending"}],"next_cursor":{"submitted_at_ms":30,"intent_id":"intent-y"},"has_more":true}`
	writeFake(t, executable, responseScriptCheckingRequest(database, result, writeProtocolVersion,
		`"operation":"owned_pending_run_intent_page"`, `"conversation_id":"conversation-1"`,
		`"before":{"submitted_at_ms":40,"intent_id":"intent-z"}`, `"limit":1`,
		`"owner":{"issuer":"https://identity.example","subject":"account-42","tenant_id":"tenant-slate"}`))
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	page, err := client.OwnedConversationPendingRunIntents(context.Background(), owner, "conversation-1",
		&intentmodel.PendingRunIntentCursor{SubmittedAtMS: 40, IntentID: "intent-z"}, 1)
	if err != nil || !page.HasMore || page.NextCursor == nil || len(page.Intents) != 1 ||
		page.NextCursor.IntentID != "intent-y" || page.Intents[0].ProfileID != "profile-1" {
		t.Fatalf("pending intent page=%#v error=%v", page, err)
	}
}

func TestOwnedConversationPendingRunIntentTimelineUsesBoundedMetadata(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "app-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"}
	result := `{"conversation_id":"conversation-1","intent_id":"intent-1","after_sequence":0,"scanned_through_sequence":1,"has_more":false,"events":[{"event_id":"event-1","seq":1,"emitted_at_ms":20,"type":"submitted"}]}`
	writeFake(t, executable, responseScriptCheckingRequest(database, result, writeProtocolVersion,
		`"operation":"owned_pending_run_intent_timeline_page"`, `"conversation_id":"conversation-1"`,
		`"intent_id":"intent-1"`, `"after_sequence":0`, `"limit":1`,
		`"owner":{"issuer":"https://identity.example","subject":"account-42","tenant_id":"tenant-slate"}`))
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	page, err := client.OwnedConversationPendingRunIntentTimeline(context.Background(), owner,
		"conversation-1", "intent-1", 0, 1)
	if err != nil || page.ScannedThroughSequence != 1 || page.HasMore || len(page.Events) != 1 ||
		page.Events[0].Type != "submitted" {
		t.Fatalf("pending intent timeline=%#v error=%v", page, err)
	}
}

func TestPendingRunIntentRequestValidationAndErrorCodeRetention(t *testing.T) {
	client := &Client{executable: "/path/that/must/not/be/run"}
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"}
	profile := intentmodel.ServerExecutionProfile{ID: "profile-1"}
	if _, err := client.SubmitOwnedPromptRunIntent(context.Background(), owner, "conversation-1", "ok",
		"key\nwith-control", 1, profile); err == nil || err.(*Error).Code != "invalid_owned_prompt_request" {
		t.Fatalf("control character idempotency key error=%v", err)
	}
	if _, err := client.OwnedConversationPendingRunIntents(context.Background(), owner, "conversation-1",
		&intentmodel.PendingRunIntentCursor{SubmittedAtMS: maxSQLiteInteger + 1, IntentID: "intent-1"}, 1); err == nil ||
		err.(*Error).Code != "invalid_pending_run_intent_request" {
		t.Fatalf("out-of-range cursor error=%v", err)
	}
	if code := stableRuntimeErrorCode("invalid_pending_run_intent_request"); code != "invalid_pending_run_intent_request" {
		t.Fatalf("pending intent error code was not retained: %q", code)
	}
	if stableRuntimeErrorCode("arbitrary-private-message") != "runtime_query_failed" {
		t.Fatal("unknown remote error text was not hidden")
	}
}

func TestPendingRunIntentResponseValidationRejectsConfusedOrExpandedDTOs(t *testing.T) {
	validReceipt := `{"prompt":{"id":"prompt-1","conversation_id":"conversation-1","role":"user","content":"do work","created_at_ms":20},"intent":{"intent_id":"intent-1","conversation_id":"conversation-1","prompt_id":"prompt-1","project_id":"project-1","profile_id":"profile-original","submitted_at_ms":20,"aggregate_version":2,"latest_sequence":1,"status":"pending"},"initial_event":{"event_id":"event-1","seq":1,"emitted_at_ms":20,"type":"submitted"},"replayed":true}`
	var receipt intentmodel.PendingRunIntentSubmissionResult
	if err := decodeStrict([]byte(validReceipt), &receipt); err != nil ||
		!validPendingRunIntentSubmission([]byte(validReceipt), receipt, "conversation-1", "do work", "profile-current") {
		t.Fatalf("replayed receipt with original profile rejected: %#v, %v", receipt, err)
	}
	newReceipt := strings.Replace(strings.Replace(validReceipt, `"profile_id":"profile-original"`,
		`"profile_id":"profile-current"`, 1), `"replayed":true`, `"replayed":false`, 1)
	if err := decodeStrict([]byte(newReceipt), &receipt); err != nil ||
		!validPendingRunIntentSubmission([]byte(newReceipt), receipt, "conversation-1", "do work", "profile-current") {
		t.Fatalf("new receipt using the selected profile rejected: %#v, %v", receipt, err)
	}
	wrongProfileReceipt := strings.Replace(newReceipt, `"profile_id":"profile-current"`,
		`"profile_id":"profile-original"`, 1)
	if err := decodeStrict([]byte(wrongProfileReceipt), &receipt); err != nil ||
		validPendingRunIntentSubmission([]byte(wrongProfileReceipt), receipt, "conversation-1", "do work", "profile-current") {
		t.Fatal("new receipt with an unrequested profile was accepted")
	}
	for _, malformed := range []string{
		strings.Replace(validReceipt, `"status":"pending"`, `"status":"running"`, 1),
		strings.Replace(validReceipt, `"latest_sequence":1`, `"latest_sequence":1,"provider":"x"`, 1),
		strings.Replace(validReceipt, `"type":"submitted"`, `"type":"completed"`, 1),
		strings.Replace(validReceipt, `"content":"do work"`, `"content":"different"`, 1),
	} {
		var malformedReceipt intentmodel.PendingRunIntentSubmissionResult
		if err := decodeStrict([]byte(malformed), &malformedReceipt); err == nil &&
			validPendingRunIntentSubmission([]byte(malformed), malformedReceipt, "conversation-1", "do work", "profile-current") {
			t.Fatalf("malformed receipt accepted: %s", malformed)
		}
	}

	assertPendingIntentPageAndTimelineValidation(t)
}

func assertPendingIntentPageAndTimelineValidation(t *testing.T) {
	t.Helper()
	validPage := `{"conversation_id":"conversation-1","intents":[{"intent_id":"intent-b","conversation_id":"conversation-1","prompt_id":"prompt-b","project_id":"project-1","profile_id":"profile-1","submitted_at_ms":20,"aggregate_version":3,"latest_sequence":1,"status":"pending"},{"intent_id":"intent-a","conversation_id":"conversation-1","prompt_id":"prompt-a","project_id":"project-1","profile_id":"profile-1","submitted_at_ms":20,"aggregate_version":2,"latest_sequence":1,"status":"pending"}],"has_more":false}`
	var page intentmodel.OwnedPendingRunIntentPage
	if err := decodeStrict([]byte(validPage), &page); err != nil ||
		!validOwnedPendingRunIntentPage([]byte(validPage), page, "conversation-1", nil, 2) {
		t.Fatalf("valid pending intent page rejected: %#v, %v", page, err)
	}
	unordered := strings.Replace(validPage, `"intent_id":"intent-a"`, `"intent_id":"intent-z"`, 1)
	if err := decodeStrict([]byte(unordered), &page); err != nil ||
		validOwnedPendingRunIntentPage([]byte(unordered), page, "conversation-1", nil, 2) {
		t.Fatal("unordered pending intent page was accepted")
	}
	pageWithDigest := strings.Replace(validPage, `"status":"pending"`, `"status":"pending","profile_sha256":[]`, 1)
	if err := decodeStrict([]byte(pageWithDigest), &page); err == nil &&
		validOwnedPendingRunIntentPage([]byte(pageWithDigest), page, "conversation-1", nil, 2) {
		t.Fatal("pending intent page leaked profile digest")
	}

	validTimeline := `{"conversation_id":"conversation-1","intent_id":"intent-1","after_sequence":0,"scanned_through_sequence":1,"has_more":false,"events":[{"event_id":"event-1","seq":1,"emitted_at_ms":20,"type":"submitted"}]}`
	var timeline intentmodel.OwnedPendingRunIntentTimelinePage
	if err := decodeStrict([]byte(validTimeline), &timeline); err != nil ||
		!validOwnedPendingRunIntentTimelinePage([]byte(validTimeline), timeline, "conversation-1", "intent-1", 0, 1) {
		t.Fatalf("valid pending intent timeline rejected: %#v, %v", timeline, err)
	}
	badTimeline := strings.Replace(validTimeline, `"type":"submitted"`, `"type":"assistant_delta"`, 1)
	if err := decodeStrict([]byte(badTimeline), &timeline); err != nil ||
		validOwnedPendingRunIntentTimelinePage([]byte(badTimeline), timeline, "conversation-1", "intent-1", 0, 1) {
		t.Fatal("payload-bearing event type was accepted")
	}
}

func TestPendingRunIntentValidationErrorCodeSurvivesRuntimeEnvelope(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "app-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	writeFake(t, executable, errorResponseScript(database, "invalid_pending_run_intent_request"))
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"}
	_, err = client.OwnedConversationPendingRunIntents(context.Background(), owner, "conversation-1", nil, 1)
	if err == nil || err.(*Error).Code != "invalid_pending_run_intent_request" {
		t.Fatalf("stable RPC error was not retained: %v", err)
	}
}

func errorResponseScript(database, code string) string {
	prefix := `{"api_version":"` + writeProtocolVersion + `","request_id":"`
	suffix := `","ok":false,"error":{"code":"` + code + `","message":"validation failed"}}`
	return fmt.Sprintf("#!/bin/sh\n[ \"$1\" = '--runtime-rpc' ] || exit 20\n[ \"$2\" = '--database' ] || exit 21\n[ \"$3\" = %s ] || exit 22\n", shellQuote(database)) +
		"IFS= read -r request || exit 23\nrequest_id=${request#*\\\"request_id\\\":\\\"}\nrequest_id=${request_id%%\\\"*}\n" +
		fmt.Sprintf("printf '%%s%%s%%s\\n' %s \"$request_id\" %s\n", shellQuote(prefix), shellQuote(suffix))
}
