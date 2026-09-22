package appserver

import (
	"net/http"
	"strconv"
	"strings"
	"testing"
)

func assertForgeConsoleNativeRunObservationRequests(
	t *testing.T, requests []recordedConversationRequest, conversationID, runID string,
) {
	t.Helper()
	timelinePath := conversationCollectionPath + "/" + conversationID + "/runs/" + runID + "/timeline"
	deviceObservationPath := conversationCollectionPath + "/" + conversationID + "/runs/" + runID + "/device-observation/preview"
	receiptObservationPath := conversationCollectionPath + "/" + conversationID + "/runs/" + runID + "/runner-receipt-observation/preview"
	want := map[recordedConversationRequest]bool{
		{method: http.MethodGet, path: conversationCollectionPath, query: "limit=50"}:                                      true,
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/prompts", query: "limit=100"}: true,
		{method: http.MethodGet, path: conversationCollectionPath + "/" + conversationID + "/runs", query: "limit=25"}:     true,
	}
	seen := make(map[recordedConversationRequest]bool, len(want))
	var timelineAfter []uint64
	deviceObservationCount := 0
	receiptObservationCount := 0
	for _, request := range requests {
		if request.path == conversationChangesPath {
			if request.method != http.MethodGet || !validCursorQuery(request.query, "after_cursor") {
				t.Fatalf("unexpected Forge native Run observation change-feed request %#v", request)
			}
			continue
		}
		if request.path == timelinePath {
			if request.method != http.MethodGet {
				t.Fatalf("unexpected Forge native Run timeline request %#v", request)
			}
			sequence, ok := parseCursorQuery(request.query, "after_sequence")
			if !ok {
				t.Fatalf("unexpected Forge native Run timeline cursor %#v", request)
			}
			timelineAfter = append(timelineAfter, sequence)
			continue
		}
		if request.path == deviceObservationPath {
			if request.method != http.MethodPost || request.query != "" {
				t.Fatalf("unexpected Forge native device observation request %#v", request)
			}
			deviceObservationCount++
			continue
		}
		if request.path == receiptObservationPath {
			if request.method != http.MethodPost || request.query != "" {
				t.Fatalf("unexpected Forge native Runner receipt observation request %#v", request)
			}
			receiptObservationCount++
			continue
		}
		if !want[request] {
			t.Fatalf("unexpected Forge native Run observation request %#v", request)
		}
		seen[request] = true
	}
	for request := range want {
		if !seen[request] {
			t.Fatalf("Forge native screen omitted Run observation request %#v; observed %#v", request, requests)
		}
	}
	if len(timelineAfter) != 2 || timelineAfter[0] != 0 || timelineAfter[1] == 0 {
		t.Fatalf("Forge native Run timeline resume cursors=%#v; want [0, positive]", timelineAfter)
	}
	if deviceObservationCount != 1 {
		t.Fatalf("Forge native device observation POST count=%d want=1; observed %#v", deviceObservationCount, requests)
	}
	if receiptObservationCount != 1 {
		t.Fatalf("Forge native Runner receipt observation POST count=%d want=1; observed %#v", receiptObservationCount, requests)
	}
}

func validCursorQuery(query, key string) bool {
	_, ok := parseCursorQuery(query, key)
	return ok
}

func parseCursorQuery(query, key string) (uint64, bool) {
	prefix := key + "="
	if !strings.HasPrefix(query, prefix) || !strings.HasSuffix(query, "&limit=128") {
		return 0, false
	}
	value := strings.TrimSuffix(strings.TrimPrefix(query, prefix), "&limit=128")
	if value == "" {
		return 0, false
	}
	cursor, err := strconv.ParseUint(value, 10, 64)
	return cursor, err == nil
}
