package appserver

import (
	"context"
	"crypto/ed25519"
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"forgeos/forge-core/internal/authn"
)

type fakeConversationBackend struct {
	err error

	changeCalls int
	changeOwner model.Owner
	changeAfter uint64
	changeLimit int
	changePage  model.OwnedConversationChangePage

	listCalls int
	listOwner model.Owner
	listAfter string
	listLimit int
	listPage  model.OwnedConversationPage

	createCalls int
	createOwner model.Owner
	createScope model.ConversationScope
	createTitle string
	createKey   string

	importCalls   int
	importOwner   model.Owner
	importTitle   string
	importPrompts []model.ConversationImportPrompt
	importKey     string
	importResult  model.OwnedConversationImportResult

	promptCalls  int
	promptOwner  model.Owner
	promptID     string
	promptCursor *model.PromptPageCursor
	promptLimit  int
	promptPage   model.ConversationPromptPage

	runCalls       int
	runOwner       model.Owner
	runID          string
	runCursor      *runmodel.OwnedRunPageCursor
	runLimit       int
	runPage        runmodel.OwnedRunPage
	timelineCalls  int
	timelineOwner  model.Owner
	timelineConvID string
	timelineRunID  string
	timelineAfter  uint64
	timelineLimit  int
	timelinePage   runmodel.OwnedRunTimelinePage

	appendCalls   int
	appendOwner   model.Owner
	appendID      string
	appendContent string
	appendKey     string
	appendVersion uint64
	appendPrompt  model.ConversationPrompt
	appendAggVer  uint64
	appendReplay  bool

	projectIdentityCalls int
	projectIdentityOwner model.Owner
	projectIdentityID    string
	projectIdentity      model.OwnedProjectConversationIdentity
}

func (backend *fakeConversationBackend) OwnedProjectConversationIdentity(
	_ context.Context,
	owner model.Owner,
	conversationID string,
) (model.OwnedProjectConversationIdentity, error) {
	backend.projectIdentityCalls++
	backend.projectIdentityOwner, backend.projectIdentityID = owner, conversationID
	if backend.err != nil {
		return model.OwnedProjectConversationIdentity{}, backend.err
	}
	return backend.projectIdentity, nil
}

func (backend *fakeConversationBackend) OwnedConversationChangesAfter(
	_ context.Context,
	owner model.Owner,
	after uint64,
	limit int,
) (model.OwnedConversationChangePage, error) {
	backend.changeCalls++
	backend.changeOwner, backend.changeAfter, backend.changeLimit = owner, after, limit
	if backend.err != nil {
		return model.OwnedConversationChangePage{}, backend.err
	}
	return backend.changePage, nil
}

func (backend *fakeConversationBackend) CreateOwnedConversation(
	_ context.Context,
	owner model.Owner,
	scope model.ConversationScope,
	title string,
	key string,
) (model.Conversation, error) {
	backend.createCalls++
	backend.createOwner, backend.createScope = owner, scope
	backend.createTitle, backend.createKey = title, key
	if backend.err != nil {
		return model.Conversation{}, backend.err
	}
	return model.Conversation{
		ID: "conversation-created", Scope: scope, Title: title, CreatedAtMS: 11, UpdatedAtMS: 11,
	}, nil
}

func (backend *fakeConversationBackend) ImportOwnedConversation(
	_ context.Context,
	owner model.Owner,
	title string,
	prompts []model.ConversationImportPrompt,
	key string,
) (model.OwnedConversationImportResult, error) {
	backend.importCalls++
	backend.importOwner, backend.importTitle = owner, title
	backend.importPrompts = append([]model.ConversationImportPrompt{}, prompts...)
	backend.importKey = key
	if backend.err != nil {
		return model.OwnedConversationImportResult{}, backend.err
	}
	return backend.importResult, nil
}

func (backend *fakeConversationBackend) ListOwnedConversations(
	_ context.Context,
	owner model.Owner,
	afterID string,
	limit int,
) (model.OwnedConversationPage, error) {
	backend.listCalls++
	backend.listOwner, backend.listAfter, backend.listLimit = owner, afterID, limit
	if backend.err != nil {
		return model.OwnedConversationPage{}, backend.err
	}
	return backend.listPage, nil
}

func (backend *fakeConversationBackend) OwnedConversationPrompts(
	_ context.Context,
	owner model.Owner,
	conversationID string,
	cursor *model.PromptPageCursor,
	limit int,
) (model.ConversationPromptPage, error) {
	backend.promptCalls++
	backend.promptOwner, backend.promptID = owner, conversationID
	backend.promptCursor, backend.promptLimit = cursor, limit
	if backend.err != nil {
		return model.ConversationPromptPage{}, backend.err
	}
	return backend.promptPage, nil
}

func (backend *fakeConversationBackend) OwnedConversationRuns(
	_ context.Context,
	owner model.Owner,
	conversationID string,
	cursor *runmodel.OwnedRunPageCursor,
	limit int,
) (runmodel.OwnedRunPage, error) {
	backend.runCalls++
	backend.runOwner, backend.runID, backend.runCursor, backend.runLimit = owner, conversationID, cursor, limit
	if backend.err != nil {
		return runmodel.OwnedRunPage{}, backend.err
	}
	return backend.runPage, nil
}

func (backend *fakeConversationBackend) OwnedConversationRunTimeline(
	_ context.Context,
	owner model.Owner,
	conversationID string,
	runID string,
	after uint64,
	limit int,
) (runmodel.OwnedRunTimelinePage, error) {
	backend.timelineCalls++
	backend.timelineOwner, backend.timelineConvID, backend.timelineRunID = owner, conversationID, runID
	backend.timelineAfter, backend.timelineLimit = after, limit
	if backend.err != nil {
		return runmodel.OwnedRunTimelinePage{}, backend.err
	}
	return backend.timelinePage, nil
}

func (backend *fakeConversationBackend) AppendOwnedPrompt(
	_ context.Context,
	owner model.Owner,
	conversationID string,
	content string,
	key string,
	expectedVersion uint64,
) (model.ConversationPrompt, uint64, bool, error) {
	backend.appendCalls++
	backend.appendOwner, backend.appendID = owner, conversationID
	backend.appendContent, backend.appendKey, backend.appendVersion = content, key, expectedVersion
	if backend.err != nil {
		return model.ConversationPrompt{}, 0, false, backend.err
	}
	return backend.appendPrompt, backend.appendAggVer, backend.appendReplay, nil
}

type conversationTestIdentity struct {
	server *httptest.Server
	key    ed25519.PrivateKey
	keyID  string
	issuer string
}

func newConversationTestIdentity(t *testing.T) (*conversationTestIdentity, *authn.Authenticator) {
	return newConversationTestIdentityWithExpectedSubject(t, "account-42")
}

func newMultiPrincipalConversationTestIdentity(t *testing.T) (*conversationTestIdentity, *authn.Authenticator) {
	return newConversationTestIdentityWithExpectedSubject(t, "")
}

func newConversationTestIdentityWithExpectedSubject(
	t *testing.T,
	expectedSubjectID string,
) (*conversationTestIdentity, *authn.Authenticator) {
	t.Helper()
	publicKey, privateKey, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	keyID := "appserver-test-key"
	keys, err := json.Marshal(map[string]any{"keys": []any{map[string]string{
		"kty": "OKP", "crv": "Ed25519", "kid": keyID, "use": "sig", "alg": "EdDSA",
		"x": base64.RawURLEncoding.EncodeToString(publicKey),
	}}})
	if err != nil {
		t.Fatal(err)
	}
	server := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/jwks" {
			http.NotFound(w, r)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write(keys)
	}))
	identity := &conversationTestIdentity{server: server, key: privateKey, keyID: keyID, issuer: server.URL}
	auth, err := authn.New(authn.Config{
		Issuer: server.URL, Audience: "forge-api", JWKSURL: server.URL + "/jwks",
		ExpectedTenantID: "tenant-slate", ExpectedSubjectID: expectedSubjectID,
		JWKSHTTPClient: server.Client(), JWKSRefreshInterval: 24 * time.Hour,
	})
	if err != nil {
		server.Close()
		t.Fatal(err)
	}
	t.Cleanup(func() {
		auth.Close()
		server.Close()
	})
	return identity, auth
}

func (identity *conversationTestIdentity) token(scopes string) string {
	return identity.tokenAs(scopes, "account-42", "tenant-slate")
}

func (identity *conversationTestIdentity) tokenAs(scopes, subject, tenant string) string {
	header, _ := json.Marshal(map[string]string{"typ": "at+jwt", "alg": "EdDSA", "kid": identity.keyID})
	claims, _ := json.Marshal(map[string]any{
		"iss": identity.issuer, "sub": subject, "aud": "forge-api",
		"exp": time.Now().Add(5 * time.Minute).Unix(), "iat": time.Now().Add(-time.Minute).Unix(),
		"tenant_id": tenant, "scope": scopes,
	})
	input := base64.RawURLEncoding.EncodeToString(header) + "." + base64.RawURLEncoding.EncodeToString(claims)
	signature := ed25519.Sign(identity.key, []byte(input))
	return input + "." + base64.RawURLEncoding.EncodeToString(signature)
}

func conversationTestHandler(t *testing.T, backend conversationBackend) (*conversationTestIdentity, http.Handler) {
	t.Helper()
	identity, auth := newConversationTestIdentity(t)
	return identity, auth.Handler(newConversationRoutesWithBackend(backend))
}
