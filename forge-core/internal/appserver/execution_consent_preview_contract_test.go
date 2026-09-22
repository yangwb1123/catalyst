package appserver

import (
	"crypto/sha256"
	"encoding/json"
	"net/http"
	"os"
	"testing"

	"forgeos/forge-core/internal/executionprofile"
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestExecutionConsentPreviewContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_EXECUTION_CONSENT_PREVIEW_FIXTURE")
	if path == "" {
		t.Skip("FORGE_EXECUTION_CONSENT_PREVIEW_FIXTURE is set by the cross-repository contract test")
	}
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var preview executionConsentPreviewResponse
	if err := json.Unmarshal(body, &preview); err != nil {
		t.Fatalf("decode execution-consent preview fixture: %v", err)
	}
	if preview.ConversationID != "conversation-001" || preview.ProjectID != "project-alpha" ||
		preview.ProfileID != "profile-reviewed-v1" ||
		preview.ProfileSHA256 != "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef" ||
		preview.MaximumTTLMS != maxExecutionConsentTTLMS {
		t.Fatalf("execution-consent preview fixture=%#v", preview)
	}
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(body, &fields); err != nil || len(fields) != 5 {
		t.Fatalf("execution-consent preview fixture fields=%d err=%v", len(fields), err)
	}
}

func TestExecutionConsentPreviewCandidateIsStrictReadOnly(t *testing.T) {
	conversationID := "conversation-consent-preview"
	projectID := "project-consent-preview"
	digest := sha256.Sum256([]byte("execution-consent-preview-profile-v1"))
	profiles, err := executionprofile.New([]executionprofile.Binding{{
		ProjectID: projectID,
		Profile: intentmodel.ServerExecutionProfile{
			ID: "profile-consent-preview-v1", SHA256: digest,
		},
	}})
	if err != nil {
		t.Fatal(err)
	}
	backend := &fakeConversationBackend{projectIdentity: model.OwnedProjectConversationIdentity{
		ConversationID: conversationID, ProjectID: projectID,
	}}
	identity, authenticator := newConversationTestIdentity(t)
	candidate := authenticator.Handler(newConversationRoutesWithInertExecutionAPI(backend, profiles))
	path := conversationCollectionPath + "/" + conversationID + "/execution-consents"

	response := requestConversationAPI(t, candidate, identity, http.MethodGet, path,
		"forge:conversations:read", "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("preview status=%d body=%q", response.Code, response.Body.String())
	}
	var preview executionConsentPreviewResponse
	if err := json.Unmarshal(response.Body.Bytes(), &preview); err != nil {
		t.Fatalf("decode preview: %v body=%q", err, response.Body.String())
	}
	if preview.ConversationID != conversationID || preview.ProjectID != projectID ||
		preview.ProfileID != "profile-consent-preview-v1" || preview.ProfileSHA256 != hexDigest(digest) ||
		preview.MaximumTTLMS != maxExecutionConsentTTLMS {
		t.Fatalf("preview=%#v", preview)
	}
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(response.Body.Bytes(), &fields); err != nil || len(fields) != 5 {
		t.Fatalf("preview fields=%v err=%v body=%q", len(fields), err, response.Body.String())
	}
	for _, field := range []string{"conversation_id", "project_id", "profile_id", "profile_sha256", "maximum_ttl_ms"} {
		if _, ok := fields[field]; !ok {
			t.Errorf("preview missing field %q", field)
		}
	}
	if backend.projectIdentityCalls != 1 || backend.projectIdentityID != conversationID {
		t.Fatalf("backend identity calls=%d id=%q", backend.projectIdentityCalls, backend.projectIdentityID)
	}
	foreign := requestConversationAPIAs(t, candidate, identity, http.MethodGet, path,
		"forge:conversations:read", "account-other", "tenant-slate", "", "", "")
	if foreign.Code != http.StatusForbidden {
		t.Fatalf("foreign owner preview status=%d body=%q", foreign.Code, foreign.Body.String())
	}
	changedDigest := digest
	changedDigest[0]++
	changedProfiles, err := executionprofile.New([]executionprofile.Binding{{
		ProjectID: projectID,
		Profile: intentmodel.ServerExecutionProfile{
			ID: "profile-consent-preview-v1", SHA256: changedDigest,
		},
	}})
	if err != nil {
		t.Fatal(err)
	}
	changedCandidate := authenticator.Handler(newConversationRoutesWithInertExecutionAPI(backend, changedProfiles))
	changed := requestConversationAPI(t, changedCandidate, identity, http.MethodGet, path,
		"forge:conversations:read", "", "", "")
	var changedPreview executionConsentPreviewResponse
	if changed.Code != http.StatusOK || json.Unmarshal(changed.Body.Bytes(), &changedPreview) != nil ||
		changedPreview.ProfileSHA256 == preview.ProfileSHA256 {
		t.Fatalf("changed profile preview status=%d preview=%#v body=%q", changed.Code, changedPreview, changed.Body.String())
	}

	for _, test := range []struct {
		name, target, body string
		want               int
	}{
		{name: "unknown query", target: path + "?profile_id=attacker", want: http.StatusBadRequest},
		{name: "body", target: path, body: `{}`, want: http.StatusBadRequest},
		{name: "write scope", target: path, want: http.StatusForbidden},
	} {
		t.Run(test.name, func(t *testing.T) {
			scopes := "forge:conversations:read"
			if test.name == "write scope" {
				scopes = "forge:conversations:write"
			}
			response := requestConversationAPI(t, candidate, identity, http.MethodGet, test.target,
				scopes, "application/json", "", test.body)
			if response.Code != test.want {
				t.Fatalf("status=%d want=%d body=%q", response.Code, test.want, response.Body.String())
			}
		})
	}

	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, profiles))
	closed := requestConversationAPI(t, production, identity, http.MethodGet, path,
		"forge:conversations:read", "", "", "")
	if closed.Code != http.StatusNotFound || closed.Body.String() != string(notFoundBody) {
		t.Fatalf("production preview status=%d body=%q", closed.Code, closed.Body.String())
	}
}

func hexDigest(value [32]byte) string {
	const hex = "0123456789abcdef"
	encoded := make([]byte, len(value)*2)
	for index, byteValue := range value {
		encoded[index*2] = hex[byteValue>>4]
		encoded[index*2+1] = hex[byteValue&0x0f]
	}
	return string(encoded)
}
