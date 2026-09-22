package appserver

import (
	"net/http"
	"strconv"
	"strings"
	"testing"
)

func assertForgeConsoleBrowserChangeCursorResumes(
	t *testing.T,
	requests []recordedConversationRequest,
) {
	t.Helper()
	var cursors []uint64
	for _, request := range requests {
		if request.path != conversationChangesPath {
			continue
		}
		parts := strings.Split(request.query, "&")
		if request.method != http.MethodGet || len(parts) != 2 ||
			!strings.HasPrefix(parts[0], "after_cursor=") || parts[1] != "limit=128" {
			t.Fatalf("unexpected Forge browser change-feed request %#v", request)
		}
		cursor, err := strconv.ParseUint(strings.TrimPrefix(parts[0], "after_cursor="), 10, 64)
		if err != nil {
			t.Fatalf("invalid Forge browser change-feed cursor in request %#v: %v", request, err)
		}
		cursors = append(cursors, cursor)
	}
	if len(cursors) < 2 || cursors[0] != 0 || cursors[1] == 0 {
		t.Fatalf("Forge browser did not prove change-feed cursor reload recovery: cursors=%v requests=%#v", cursors, requests)
	}
}
