package runtimebridge

import (
	"context"
	"fmt"
	auditprojection "forgeos/forge-core/internal/auditprojection"
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
	"path/filepath"
	"strings"
	"testing"
)

func TestOwnedConversationRunsUsesRuntimeV2AndExactOwner(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "app-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"}
	result := `{"conversation_id":"conversation-1","runs":[{"run_id":"run-2","prompt_id":"prompt-2","created_at_ms":20,"latest_sequence":3,"status":"nonterminal"}],"next_cursor":{"created_at_ms":20,"run_id":"run-2"},"has_more":true}`
	writeFake(t, executable, responseScriptCheckingRequest(database, result, writeProtocolVersion,
		`"api_version":"forgeos.runtime-bridge/v2"`, `"operation":"owned_run_page"`,
		`"conversation_id":"conversation-1"`, `"before_created_at_ms":21`, `"before_run_id":"run-3"`, `"limit":1`,
		`"owner":{"issuer":"https://identity.example","subject":"account-42","tenant_id":"tenant-slate"}`))
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	cursor := &runmodel.OwnedRunPageCursor{CreatedAtMS: 21, RunID: "run-3"}
	page, err := client.OwnedConversationRuns(context.Background(), owner, "conversation-1", cursor, 1)
	if err != nil || !page.HasMore || page.NextCursor == nil || len(page.Runs) != 1 ||
		page.NextCursor.RunID != "run-2" || page.Runs[0].Status != "nonterminal" {
		t.Fatalf("Run page=%#v error=%v", page, err)
	}
	if _, err := client.OwnedConversationRuns(context.Background(), owner, "conversation-1", nil, maxOwnedRunPageLimit+1); err == nil {
		t.Fatal("over-limit Run page request was accepted")
	}
}

func TestOwnedConversationRunTimelineUsesBoundedMetadataOnlyPage(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "app-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"}
	result := `{"conversation_id":"conversation-1","run_id":"run-2","after_sequence":2,"scanned_through_sequence":3,"has_more":false,"events":[{"seq":3,"emitted_at_ms":30,"type":"turn_started"}]}`
	writeFake(t, executable, responseScriptCheckingRequest(database, result, writeProtocolVersion,
		`"operation":"owned_run_timeline_page"`, `"conversation_id":"conversation-1"`, `"run_id":"run-2"`,
		`"after_sequence":2`, `"limit":8`,
		`"owner":{"issuer":"https://identity.example","subject":"account-42","tenant_id":"tenant-slate"}`))
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	page, err := client.OwnedConversationRunTimeline(context.Background(), owner, "conversation-1", "run-2", 2, 8)
	if err != nil || page.ScannedThroughSequence != 3 || len(page.Events) != 1 || page.Events[0].Type != "turn_started" {
		t.Fatalf("Run timeline=%#v error=%v", page, err)
	}
	if _, err := client.OwnedConversationRunTimeline(context.Background(), owner, "conversation-1", "run-2", 0, maxOwnedRunTimelinePageLimit+1); err == nil {
		t.Fatal("over-limit Run timeline request was accepted")
	}
}

func TestOwnedConversationRunObservationUsesRuntimeV2AndClosedAuthority(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "app-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"}
	result := fmt.Sprintf(`{"api_version":"forge.run.observed.v1","owner_ref":%q,"conversation_id":"conversation-1","run_id":"run-2","prompt_id":"prompt-2","created_at_ms":20,"latest_sequence":3,"status":"nonterminal","metadata_observed":true,"content_included":false,"authority":{"identity_verified":false,"owner_authorized":false,"run_authoritative":false,"persistence_attested":false,"content_provenance_verified":false,"reservation_created":false,"execution_authorized":false,"dispatch_performed":false}}`, auditprojection.ObservedOwnerReference(owner))
	writeFake(t, executable, responseScriptCheckingRequest(database, result, writeProtocolVersion,
		`"operation":"owned_run_observation"`, `"conversation_id":"conversation-1"`, `"run_id":"run-2"`,
		`"owner":{"issuer":"https://identity.example","subject":"account-42","tenant_id":"tenant-slate"}`))
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	observed, err := client.OwnedConversationRunObservation(context.Background(), owner, "conversation-1", "run-2")
	if err != nil || observed.APIVersion != auditprojection.ForgeRunObservedV1 || observed.RunID != "run-2" ||
		!observed.MetadataObserved || observed.ContentIncluded || observed.Authority.OwnerAuthorized || observed.Authority.DispatchPerformed {
		t.Fatalf("Run observation=%#v error=%v", observed, err)
	}
}

func TestOwnedRunResponsesRejectPayloadLeaksAndCursorGaps(t *testing.T) {
	validRuns := `{"conversation_id":"c1","runs":[{"run_id":"run-b","prompt_id":"p2","created_at_ms":20,"latest_sequence":2,"status":"completed"},{"run_id":"run-a","prompt_id":"p1","created_at_ms":20,"latest_sequence":1,"status":"nonterminal"}],"has_more":false}`
	var runs runmodel.OwnedRunPage
	if err := decodeStrict([]byte(validRuns), &runs); err != nil ||
		!validOwnedRunPage([]byte(validRuns), runs, "c1", nil, 2) {
		t.Fatalf("valid Run page rejected: %#v, %v", runs, err)
	}
	for _, malformed := range []string{
		strings.Replace(validRuns, `"status":"completed"`, `"status":"running"`, 1),
		strings.Replace(validRuns, `"latest_sequence":2`, `"latest_sequence":2,"execution_json":"{}"`, 1),
		strings.Replace(validRuns, `"run_id":"run-a"`, `"run_id":"run-z"`, 1),
		strings.Replace(validRuns, `"run_id":"run-a","prompt_id":"p1","created_at_ms":20`, `"run_id":"run-b","prompt_id":"p1","created_at_ms":19`, 1),
	} {
		var page runmodel.OwnedRunPage
		if err := decodeStrict([]byte(malformed), &page); err == nil && validOwnedRunPage([]byte(malformed), page, "c1", nil, 2) {
			t.Fatalf("malformed Run page accepted: %s", malformed)
		}
	}
	shortRunPage := []byte(`{"conversation_id":"c1","runs":[{"run_id":"run-b","prompt_id":"p2","created_at_ms":20,"latest_sequence":2,"status":"completed"}],"next_cursor":{"created_at_ms":20,"run_id":"run-b"},"has_more":true}`)
	if err := decodeStrict(shortRunPage, &runs); err != nil || !validOwnedRunPage(shortRunPage, runs, "c1", nil, 2) {
		t.Fatalf("valid short byte-budget Run page rejected: %#v, %v", runs, err)
	}
	validTimeline := `{"conversation_id":"c1","run_id":"r1","after_sequence":4,"scanned_through_sequence":5,"has_more":false,"events":[{"seq":5,"emitted_at_ms":10,"type":"activity"}]}`
	var timeline runmodel.OwnedRunTimelinePage
	if err := decodeStrict([]byte(validTimeline), &timeline); err != nil ||
		!validOwnedRunTimelinePage([]byte(validTimeline), timeline, "c1", "r1", 4, 1) {
		t.Fatalf("valid Run timeline rejected: %#v, %v", timeline, err)
	}
	for _, malformed := range []string{
		strings.Replace(validTimeline, `"seq":5`, `"seq":6`, 1),
		strings.Replace(validTimeline, `"type":"activity"`, `"type":"assistant_delta"`, 1),
		strings.Replace(validTimeline, `"type":"activity"`, `"type":"activity","output":"secret"`, 1),
		strings.Replace(validTimeline, `"emitted_at_ms":10`, `"emitted_at_ms":10,"output":"secret"`, 1),
	} {
		var page runmodel.OwnedRunTimelinePage
		if err := decodeStrict([]byte(malformed), &page); err == nil &&
			validOwnedRunTimelinePage([]byte(malformed), page, "c1", "r1", 4, 1) {
			t.Fatalf("malformed Run timeline accepted: %s", malformed)
		}
	}
	shortTimeline := []byte(`{"conversation_id":"c1","run_id":"r1","after_sequence":4,"scanned_through_sequence":5,"has_more":true,"events":[{"seq":5,"emitted_at_ms":10,"type":"activity"}]}`)
	if err := decodeStrict(shortTimeline, &timeline); err != nil ||
		!validOwnedRunTimelinePage(shortTimeline, timeline, "c1", "r1", 4, 2) {
		t.Fatalf("valid short byte-budget Run timeline rejected: %#v, %v", timeline, err)
	}
}

func TestOwnedRunObservationResponsesRejectAuthorityAndWireShapeChanges(t *testing.T) {
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"}
	valid := []byte(fmt.Sprintf(`{"api_version":"forge.run.observed.v1","owner_ref":%q,"conversation_id":"c1","run_id":"r1","prompt_id":"p1","created_at_ms":20,"latest_sequence":2,"status":"completed","metadata_observed":true,"content_included":false,"authority":{"identity_verified":false,"owner_authorized":false,"run_authoritative":false,"persistence_attested":false,"content_provenance_verified":false,"reservation_created":false,"execution_authorized":false,"dispatch_performed":false}}`, auditprojection.ObservedOwnerReference(owner)))
	var observed auditprojection.RunObserved
	if err := decodeStrict(valid, &observed); err != nil || !validOwnedRunObservation(valid, observed, owner, "c1", "r1") {
		t.Fatalf("valid Run observation rejected: %#v, %v", observed, err)
	}
	for _, malformed := range [][]byte{
		[]byte(strings.Replace(string(valid), `"content_included":false`, `"content_included":true`, 1)),
		[]byte(strings.Replace(string(valid), `"dispatch_performed":false`, `"dispatch_performed":true`, 1)),
		[]byte(strings.Replace(string(valid), `"status":"completed"`, `"status":"running"`, 1)),
		[]byte(strings.Replace(string(valid), `"authority":{`, `"authority":{"extra":false,`, 1)),
		append(append([]byte{}, valid...), []byte(` {}`)...),
	} {
		var candidate auditprojection.RunObserved
		if err := decodeStrict(malformed, &candidate); err == nil && validOwnedRunObservation(malformed, candidate, owner, "c1", "r1") {
			t.Fatalf("malformed Run observation accepted: %s", malformed)
		}
	}
}

func TestOwnedRunNumbersUseJSONSafeIntegerBoundary(t *testing.T) {
	const maxSafe = maxSafeJSONInteger
	validRuns := fmt.Sprintf(`{"conversation_id":"c1","runs":[{"run_id":"run-b","prompt_id":"p2","created_at_ms":%d,"latest_sequence":%d,"status":"completed"}],"has_more":false}`, maxSafe, maxSafe)
	var page runmodel.OwnedRunPage
	if err := decodeStrict([]byte(validRuns), &page); err != nil || !validOwnedRunPage([]byte(validRuns), page, "c1", nil, 1) {
		t.Fatalf("JSON-safe Run summary boundary rejected: %#v, %v", page, err)
	}
	for _, oversized := range []string{
		strings.Replace(validRuns, fmt.Sprintf(`"created_at_ms":%d`, maxSafe), fmt.Sprintf(`"created_at_ms":%d`, maxSafe+1), 1),
		strings.Replace(validRuns, fmt.Sprintf(`"latest_sequence":%d`, maxSafe), fmt.Sprintf(`"latest_sequence":%d`, maxSafe+1), 1),
	} {
		var oversizedPage runmodel.OwnedRunPage
		if err := decodeStrict([]byte(oversized), &oversizedPage); err == nil && validOwnedRunPage([]byte(oversized), oversizedPage, "c1", nil, 1) {
			t.Fatalf("Run summary above JSON-safe integer accepted: %s", oversized)
		}
	}

	validTimeline := fmt.Sprintf(`{"conversation_id":"c1","run_id":"r1","after_sequence":%d,"scanned_through_sequence":%d,"has_more":false,"events":[]}`, maxSafe, maxSafe)
	var timeline runmodel.OwnedRunTimelinePage
	if err := decodeStrict([]byte(validTimeline), &timeline); err != nil || !validOwnedRunTimelinePage([]byte(validTimeline), timeline, "c1", "r1", maxSafe, 1) {
		t.Fatalf("JSON-safe Run timeline boundary rejected: %#v, %v", timeline, err)
	}
	owner := model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"}
	if _, err := (&Client{}).OwnedConversationRuns(context.Background(), owner, "c1", &runmodel.OwnedRunPageCursor{
		CreatedAtMS: maxSafe + 1,
		RunID:       "run-1",
	}, 1); err == nil {
		t.Fatal("Run page cursor above JSON-safe integer accepted")
	}
	if _, err := (&Client{}).OwnedConversationRunTimeline(context.Background(), owner, "c1", "r1", maxSafe+1, 1); err == nil {
		t.Fatal("Run timeline request above JSON-safe integer accepted")
	}
}
