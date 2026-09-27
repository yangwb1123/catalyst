package appserver

import (
	"fmt"
	"net/http"
	"strings"
	"testing"

	"forgeos/forge-core/internal/executionattempt"
)

// The Attempt boundary is a read-only POST candidate. Keep its method/query
// gate and missing-proof response explicit so a client cannot accidentally
// turn a preview into an unbounded or ambiguous request.
func TestRunnerAttemptBoundaryPreviewRejectsMethodQueryAndUnknownLease(t *testing.T) {
	fixture := newRunnerAttemptBoundaryRouteFixture(t)
	path := fmt.Sprintf(runnerAttemptBoundaryPath, "conversation-1", "run-1")
	body := runnerAttemptBoundaryBody(t, fixture.owner, executionattempt.BeginStarting)

	get := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodGet, path,
		runnerTransportAdmissionScope, "", "", "")
	if get.Code != http.StatusMethodNotAllowed || get.Header().Get("Allow") != http.MethodPost {
		t.Fatalf("GET status=%d allow=%q body=%q", get.Code, get.Header().Get("Allow"), get.Body.String())
	}

	query := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, path+"?probe=1",
		runnerTransportAdmissionScope, "application/json", "", body)
	if query.Code != http.StatusBadRequest || !strings.Contains(query.Body.String(), `"code":"invalid_query"`) {
		t.Fatalf("query status=%d body=%q", query.Code, query.Body.String())
	}

	unknownLease := strings.Replace(body, `"attempt_id":"attempt-1"`, `"attempt_id":"attempt-missing"`, 2)
	missing := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, path,
		runnerTransportAdmissionScope, "application/json", "", unknownLease)
	if missing.Code != http.StatusConflict || !strings.Contains(missing.Body.String(), `"code":"lease_not_found"`) {
		t.Fatalf("unknown lease status=%d body=%q", missing.Code, missing.Body.String())
	}
}
