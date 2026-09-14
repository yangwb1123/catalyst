package appserver

import (
	"encoding/json"
	"fmt"
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/executionprofile"
	"forgeos/forge-core/internal/runtimebridge"
)

func TestInertExecutionHTTPToRustHubWhenConfigured(t *testing.T) {
	fixture := newExecutionHTTPFixture(t)
	conversationID := createExecutionProjectConversation(t, fixture)
	preview := assertExecutionProfilePreview(t, fixture, conversationID)
	consentPath := conversationCollectionPath + "/" + url.PathEscape(conversationID) + "/execution-consents"
	assertStaleExecutionProfileConfirmations(t, fixture, consentPath, preview)
	grantID := grantExecutionProfile(t, fixture, consentPath, preview)
	intentPath := conversationCollectionPath + "/" + url.PathEscape(conversationID) + "/run-intents"
	intentBody := `{"content":"compute the approved task","expected_version":1}`
	submitted := submitExecutionIntent(t, fixture, intentPath, intentBody)
	assertExecutionIntentReadViews(t, fixture, conversationID, intentPath, submitted)
	assertExecutionRevocationAndReplay(t, fixture, consentPath, intentPath, intentBody, grantID)
	assertForeignIntentReadRejected(t, fixture, intentPath)
	assertProductionIntentRouteClosed(t, fixture, intentPath)
}

type executionHTTPFixture struct {
	client       *runtimebridge.Client
	profiles     *executionprofile.Catalog
	identity     *conversationTestIdentity
	handler      http.Handler
	authenticate func(http.Handler) http.Handler
	projectID    string
}

func newExecutionHTTPFixture(t *testing.T) executionHTTPFixture {
	t.Helper()
	executable := os.Getenv("FORGE_RUNTIME_BIN")
	if executable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for Go HTTP to Rust Hub integration")
	}
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	if err := os.Mkdir(runtimeState, 0o700); err != nil {
		t.Fatal(err)
	}
	initializeRuntimeHubForIntegration(t, executable, runtimeState)
	_, projectID, _ := seedRuntimeConversationScopes(t, executable, runtimeState)
	appState := filepath.Join(t.TempDir(), "app-server-state")
	if err := os.Mkdir(appState, 0o700); err != nil {
		t.Fatal(err)
	}
	client, err := runtimebridge.New(runtimebridge.Config{
		Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState,
	})
	if err != nil {
		t.Fatal(err)
	}
	var profileDigest [32]byte
	for index := range profileDigest {
		profileDigest[index] = byte(index)
	}
	profiles, err := executionprofile.New([]executionprofile.Binding{{
		ProjectID: projectID,
		Profile:   intentmodel.ServerExecutionProfile{ID: "profile-reviewed-v1", SHA256: profileDigest},
	}})
	if err != nil {
		t.Fatal(err)
	}
	identity, authenticator := newConversationTestIdentity(t)
	authenticate := authenticator.Handler
	return executionHTTPFixture{
		client: client, profiles: profiles, identity: identity,
		handler:      authenticate(newConversationRoutesWithInertExecutionAPI(client, profiles)),
		authenticate: authenticate, projectID: projectID,
	}
}

func createExecutionProjectConversation(t *testing.T, fixture executionHTTPFixture) string {
	t.Helper()
	created := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, conversationCollectionPath,
		"forge:conversations:write", "application/json", "intent-test-conversation",
		fmt.Sprintf(`{"scope":{"kind":"project","id":%q},"title":"private execution test"}`, fixture.projectID))
	var conversation model.Conversation
	if err := json.Unmarshal(created.Body.Bytes(), &conversation); err != nil ||
		created.Code != http.StatusCreated || conversation.ID == "" || conversation.Scope.ID != fixture.projectID {
		t.Fatalf("create status=%d conversation=%#v body=%q decode=%v",
			created.Code, conversation, created.Body.String(), err)
	}
	global := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, conversationCollectionPath,
		"forge:conversations:write", "application/json", "intent-test-global",
		`{"scope":{"kind":"global"},"title":"global session cannot run project intent"}`)
	var globalConversation model.Conversation
	if err := json.Unmarshal(global.Body.Bytes(), &globalConversation); err != nil || global.Code != http.StatusCreated {
		t.Fatalf("create global conversation status=%d body=%q decode=%v", global.Code, global.Body.String(), err)
	}
	preview := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodGet,
		conversationCollectionPath+"/"+url.PathEscape(globalConversation.ID)+"/execution-consents",
		"forge:conversations:read", "", "", "")
	if preview.Code != http.StatusConflict {
		t.Fatalf("global Conversation consent preview status=%d body=%q", preview.Code, preview.Body.String())
	}
	return conversation.ID
}

func assertExecutionProfilePreview(
	t *testing.T,
	fixture executionHTTPFixture,
	conversationID string,
) executionConsentPreviewResponse {
	t.Helper()
	path := conversationCollectionPath + "/" + url.PathEscape(conversationID) + "/execution-consents"
	response := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodGet, path,
		"forge:conversations:read", "", "", "")
	var preview executionConsentPreviewResponse
	var digest [32]byte
	for index := range digest {
		digest[index] = byte(index)
	}
	if err := json.Unmarshal(response.Body.Bytes(), &preview); err != nil || response.Code != http.StatusOK ||
		preview.ConversationID != conversationID || preview.ProjectID != fixture.projectID ||
		preview.ProfileID != "profile-reviewed-v1" || preview.ProfileSHA256 != fmt.Sprintf("%x", digest) ||
		preview.MaximumTTLMS != maxExecutionConsentTTLMS {
		t.Fatalf("consent preview status=%d preview=%#v body=%q decode=%v",
			response.Code, preview, response.Body.String(), err)
	}
	return preview
}

func assertStaleExecutionProfileConfirmations(
	t *testing.T,
	fixture executionHTTPFixture,
	consentPath string,
	preview executionConsentPreviewResponse,
) {
	t.Helper()
	wrongScope := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, consentPath,
		"forge:conversations:read", "application/json", "intent-test-wrong-scope",
		`{"confirm_execution_profile":true,"expected_project_id":"p1","expected_profile_id":"profile-reviewed-v1","expected_profile_sha256":"0000000000000000000000000000000000000000000000000000000000000000","expires_at_ms":1}`)
	if wrongScope.Code != http.StatusForbidden {
		t.Fatalf("grant with read-only scope status=%d body=%q", wrongScope.Code, wrongScope.Body.String())
	}
	grantBody := executionGrantBody(preview, uint64(time.Now().Add(time.Hour).UnixMilli()))
	staleBodies := []string{
		strings.Replace(grantBody, fmt.Sprintf(`"expected_project_id":%q`, preview.ProjectID), `"expected_project_id":"another-project"`, 1),
		strings.Replace(grantBody, fmt.Sprintf(`"expected_profile_id":%q`, preview.ProfileID), `"expected_profile_id":"another-profile"`, 1),
	}
	for index, body := range staleBodies {
		response := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, consentPath,
			"forge:conversations:write", "application/json", fmt.Sprintf("intent-test-stale-%d", index), body)
		if response.Code != http.StatusConflict || !strings.Contains(response.Body.String(), "execution_profile_changed") {
			t.Errorf("stale confirmation status=%d body=%q", response.Code, response.Body.String())
		}
	}
	assertChangedExecutionProfileRejected(t, fixture, consentPath, grantBody)
}

func executionGrantBody(preview executionConsentPreviewResponse, expiry uint64) string {
	return fmt.Sprintf(`{"confirm_execution_profile":true,"expected_project_id":%q,"expected_profile_id":%q,"expected_profile_sha256":%q,"expires_at_ms":%d}`,
		preview.ProjectID, preview.ProfileID, preview.ProfileSHA256, expiry)
}

func assertChangedExecutionProfileRejected(t *testing.T, fixture executionHTTPFixture, consentPath, grantBody string) {
	t.Helper()
	var digest [32]byte
	for index := range digest {
		digest[index] = byte(index)
	}
	digest[0]++
	changed, err := executionprofile.New([]executionprofile.Binding{{
		ProjectID: fixture.projectID,
		Profile:   intentmodel.ServerExecutionProfile{ID: "profile-reviewed-v1", SHA256: digest},
	}})
	if err != nil {
		t.Fatal(err)
	}
	handler := fixture.authenticate(newConversationRoutesWithInertExecutionAPI(fixture.client, changed))
	response := requestConversationAPI(t, handler, fixture.identity, http.MethodPost, consentPath,
		"forge:conversations:write", "application/json", "intent-test-profile-changed", grantBody)
	if response.Code != http.StatusConflict || !strings.Contains(response.Body.String(), "execution_profile_changed") {
		t.Fatalf("grant after profile change status=%d body=%q", response.Code, response.Body.String())
	}
}

func grantExecutionProfile(
	t *testing.T,
	fixture executionHTTPFixture,
	consentPath string,
	preview executionConsentPreviewResponse,
) string {
	t.Helper()
	expiry := uint64(time.Now().Add(time.Hour).UnixMilli())
	grantBody := executionGrantBody(preview, expiry)
	response := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, consentPath,
		"forge:conversations:write", "application/json", "intent-test-consent", grantBody)
	var grant executionConsentGrantResponse
	if err := json.Unmarshal(response.Body.Bytes(), &grant); err != nil || response.Code != http.StatusCreated ||
		grant.Grant.GrantID == "" || grant.Grant.ProjectID != fixture.projectID ||
		grant.Grant.ProfileID != preview.ProfileID || grant.Grant.ProfileSHA256 != preview.ProfileSHA256 || grant.Replayed {
		t.Fatalf("grant status=%d grant=%#v body=%q decode=%v", response.Code, grant, response.Body.String(), err)
	}
	assertGrantReplayAndInputRejection(t, fixture, consentPath, grantBody, grant.Grant.GrantID)
	return grant.Grant.GrantID
}

func assertGrantReplayAndInputRejection(
	t *testing.T,
	fixture executionHTTPFixture,
	consentPath, grantBody, grantID string,
) {
	t.Helper()
	replay := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, consentPath,
		"forge:conversations:write", "application/json", "intent-test-consent", grantBody)
	var replayed executionConsentGrantResponse
	if err := json.Unmarshal(replay.Body.Bytes(), &replayed); err != nil || replay.Code != http.StatusOK ||
		!replayed.Replayed || replayed.Grant.GrantID != grantID {
		t.Fatalf("grant replay status=%d grant=%#v body=%q decode=%v", replay.Code, replayed, replay.Body.String(), err)
	}
	injected := strings.TrimSuffix(grantBody, "}") + `,"profile_id":"attacker-profile"}`
	bad := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, consentPath,
		"forge:conversations:write", "application/json", "intent-test-consent-extra", injected)
	if bad.Code != http.StatusBadRequest {
		t.Fatalf("client-selected profile status=%d body=%q", bad.Code, bad.Body.String())
	}
	changed := strings.Replace(grantBody, fmt.Sprintf(`"expires_at_ms":%d`, parseGrantExpiry(grantBody)),
		fmt.Sprintf(`"expires_at_ms":%d`, parseGrantExpiry(grantBody)+1), 1)
	conflict := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, consentPath,
		"forge:conversations:write", "application/json", "intent-test-consent", changed)
	if conflict.Code != http.StatusConflict {
		t.Fatalf("changed consent key status=%d body=%q", conflict.Code, conflict.Body.String())
	}
}

func parseGrantExpiry(body string) uint64 {
	var request grantExecutionConsentRequest
	_ = json.Unmarshal([]byte(body), &request)
	return request.ExpiresAtMS
}

func submitExecutionIntent(
	t *testing.T,
	fixture executionHTTPFixture,
	intentPath, intentBody string,
) intentmodel.PendingRunIntentSubmissionResult {
	t.Helper()
	injected := `{"content":"compute the approved task","expected_version":1,"profile_id":"attacker-profile"}`
	bad := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, intentPath,
		"forge:conversations:write", "application/json", "intent-test-profile-injection", injected)
	if bad.Code != http.StatusBadRequest {
		t.Fatalf("client-selected intent profile status=%d body=%q", bad.Code, bad.Body.String())
	}
	response := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, intentPath,
		"forge:conversations:write", "application/json", "intent-test-submit", intentBody)
	var submitted intentmodel.PendingRunIntentSubmissionResult
	if err := json.Unmarshal(response.Body.Bytes(), &submitted); err != nil || response.Code != http.StatusCreated ||
		submitted.Replayed || submitted.Prompt.ID == "" || submitted.Prompt.Content != "compute the approved task" ||
		submitted.Intent.Status != "pending" || submitted.Intent.ProfileID != "profile-reviewed-v1" ||
		submitted.Intent.ProjectID != fixture.projectID || submitted.InitialEvent.Type != "submitted" || submitted.InitialEvent.Sequence != 1 {
		t.Fatalf("submit status=%d result=%#v body=%q decode=%v", response.Code, submitted, response.Body.String(), err)
	}
	assertIntentReplay(t, fixture, intentPath, intentBody, submitted)
	return submitted
}

func assertIntentReplay(
	t *testing.T,
	fixture executionHTTPFixture,
	intentPath, intentBody string,
	submitted intentmodel.PendingRunIntentSubmissionResult,
) {
	t.Helper()
	response := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, intentPath,
		"forge:conversations:write", "application/json", "intent-test-submit", intentBody)
	var replay intentmodel.PendingRunIntentSubmissionResult
	if err := json.Unmarshal(response.Body.Bytes(), &replay); err != nil || response.Code != http.StatusOK ||
		!replay.Replayed || replay.Prompt.ID != submitted.Prompt.ID || replay.Intent.IntentID != submitted.Intent.IntentID {
		t.Fatalf("intent replay status=%d result=%#v body=%q decode=%v", response.Code, replay, response.Body.String(), err)
	}
}

func assertExecutionIntentReadViews(
	t *testing.T,
	fixture executionHTTPFixture,
	conversationID, intentPath string,
	submitted intentmodel.PendingRunIntentSubmissionResult,
) {
	t.Helper()
	response := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodGet, intentPath,
		"forge:conversations:read", "", "", "")
	var page intentmodel.OwnedPendingRunIntentPage
	if err := json.Unmarshal(response.Body.Bytes(), &page); err != nil || response.Code != http.StatusOK ||
		len(page.Intents) != 1 || page.Intents[0].IntentID != submitted.Intent.IntentID ||
		strings.Contains(response.Body.String(), "compute the approved task") || strings.Contains(response.Body.String(), "profile_sha256") {
		t.Fatalf("intent page status=%d page=%#v body=%q decode=%v", response.Code, page, response.Body.String(), err)
	}
	assertIntentReadBoundaries(t, fixture, conversationID, intentPath, submitted.Intent.IntentID)
}

func assertIntentReadBoundaries(t *testing.T, fixture executionHTTPFixture, conversationID, intentPath, intentID string) {
	t.Helper()
	for _, path := range []string{intentPath + "?before_submitted_at_ms=1", intentPath + "?limit=26"} {
		response := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodGet, path,
			"forge:conversations:read", "", "", "")
		if response.Code != http.StatusBadRequest {
			t.Errorf("invalid intent query %q status=%d body=%q", path, response.Code, response.Body.String())
		}
	}
	denied := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodGet, intentPath,
		"forge:conversations:write", "", "", "")
	if denied.Code != http.StatusForbidden {
		t.Fatalf("intent page with write-only scope status=%d body=%q", denied.Code, denied.Body.String())
	}
	assertIntentTimelineAndNoRun(t, fixture, conversationID, intentPath, intentID)
}

func assertIntentTimelineAndNoRun(t *testing.T, fixture executionHTTPFixture, conversationID, intentPath, intentID string) {
	t.Helper()
	timelinePath := intentPath + "/" + url.PathEscape(intentID) + "/timeline"
	response := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodGet, timelinePath,
		"forge:conversations:read", "", "", "")
	var timeline intentmodel.OwnedPendingRunIntentTimelinePage
	if err := json.Unmarshal(response.Body.Bytes(), &timeline); err != nil || response.Code != http.StatusOK ||
		len(timeline.Events) != 1 || timeline.Events[0].Type != "submitted" || timeline.Events[0].Sequence != 1 ||
		strings.Contains(response.Body.String(), "compute the approved task") {
		t.Fatalf("intent timeline status=%d page=%#v body=%q decode=%v", response.Code, timeline, response.Body.String(), err)
	}
	runResponse := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodGet,
		conversationCollectionPath+"/"+url.PathEscape(conversationID)+"/runs?limit=25",
		"forge:conversations:read", "", "", "")
	var runs runmodel.OwnedRunPage
	if err := json.Unmarshal(runResponse.Body.Bytes(), &runs); err != nil || runResponse.Code != http.StatusOK ||
		runs.Runs == nil || len(runs.Runs) != 0 {
		t.Fatalf("pending intent created a normal Run: status=%d runs=%#v body=%q decode=%v",
			runResponse.Code, runs, runResponse.Body.String(), err)
	}
}

func assertExecutionRevocationAndReplay(
	t *testing.T,
	fixture executionHTTPFixture,
	consentPath, intentPath, intentBody, grantID string,
) {
	t.Helper()
	revoke := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodDelete,
		executionConsentCollectionPath+"/"+url.PathEscape(grantID),
		"forge:conversations:write", "", "intent-test-revoke", "")
	if revoke.Code != http.StatusOK || strings.Contains(revoke.Body.String(), "profile_sha256") {
		t.Fatalf("consent revoke status=%d body=%q", revoke.Code, revoke.Body.String())
	}
	newIntent := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, intentPath,
		"forge:conversations:write", "application/json", "intent-test-after-revoke",
		`{"content":"try after revoke","expected_version":2}`)
	if newIntent.Code != http.StatusConflict {
		t.Fatalf("intent after revocation status=%d body=%q", newIntent.Code, newIntent.Body.String())
	}
	replay := requestConversationAPI(t, fixture.handler, fixture.identity, http.MethodPost, intentPath,
		"forge:conversations:write", "application/json", "intent-test-submit", intentBody)
	if replay.Code != http.StatusOK || !strings.Contains(replay.Body.String(), `"replayed":true`) {
		t.Fatalf("original receipt after revocation status=%d body=%q", replay.Code, replay.Body.String())
	}
}

func assertForeignIntentReadRejected(t *testing.T, fixture executionHTTPFixture, intentPath string) {
	t.Helper()
	foreign := requestConversationAPIAs(t, fixture.handler, fixture.identity, http.MethodGet, intentPath,
		"forge:conversations:read", "account-other", "tenant-slate", "", "", "")
	if foreign.Code != http.StatusForbidden {
		t.Fatalf("foreign owner intent read status=%d body=%q", foreign.Code, foreign.Body.String())
	}
}

func assertProductionIntentRouteClosed(t *testing.T, fixture executionHTTPFixture, intentPath string) {
	t.Helper()
	productionSurface := fixture.authenticate(newConversationRoutes(fixture.client))
	response := requestConversationAPI(t, productionSurface, fixture.identity, http.MethodGet, intentPath,
		"forge:conversations:read", "", "", "")
	if response.Code != http.StatusNotFound {
		t.Fatalf("production pending-intent path must remain closed: status=%d body=%q", response.Code, response.Body.String())
	}
}
