package appserver

import (
	"crypto/ed25519"
	"encoding/base64"
	"encoding/json"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"io"
	"net/http"
	"strings"
	"testing"
	"time"
)

func createSharedConversationAsClientA(
	t *testing.T,
	client *http.Client,
	baseURL, token string,
) model.Conversation {
	t.Helper()
	createdResponse := doConversationClientRequest(t, client, baseURL, token,
		http.MethodPost, conversationCollectionPath, "application/json", "client-a-create",
		`{"scope":{"kind":"global"},"title":"Shared from client A"}`)
	if createdResponse.StatusCode != http.StatusCreated {
		t.Fatalf("client A create status=%d body=%q", createdResponse.StatusCode, readConversationClientBody(t, createdResponse))
	}
	var created model.Conversation
	if err := json.NewDecoder(createdResponse.Body).Decode(&created); err != nil || created.ID == "" {
		t.Fatalf("client A create response=%#v decode=%v", created, err)
	}
	_ = createdResponse.Body.Close()
	return created
}

func assertConversationVisibleToClientB(
	t *testing.T,
	client *http.Client,
	baseURL, token string,
	created model.Conversation,
) {
	t.Helper()
	listResponse := doConversationClientRequest(t, client, baseURL, token,
		http.MethodGet, conversationCollectionPath+"?limit=50", "", "", "")
	if listResponse.StatusCode != http.StatusOK {
		t.Fatalf("client B list status=%d body=%q", listResponse.StatusCode, readConversationClientBody(t, listResponse))
	}
	var page model.OwnedConversationPage
	if err := json.NewDecoder(listResponse.Body).Decode(&page); err != nil || len(page.Conversations) != 1 ||
		page.Conversations[0].Conversation.ID != created.ID {
		t.Fatalf("client B list=%#v decode=%v", page, err)
	}
	_ = listResponse.Body.Close()
}

func appendPromptAsClientB(t *testing.T, client *http.Client, baseURL, token, promptPath string) {
	t.Helper()
	appendResponse := doConversationClientRequest(t, client, baseURL, token,
		http.MethodPost, promptPath, "application/json", "client-b-prompt",
		`{"content":"prompt sent from client B","expected_version":1}`)
	if appendResponse.StatusCode != http.StatusCreated {
		t.Fatalf("client B append status=%d body=%q", appendResponse.StatusCode, readConversationClientBody(t, appendResponse))
	}
	_ = appendResponse.Body.Close()
}

func assertPromptVisibleToClientA(
	t *testing.T,
	client *http.Client,
	baseURL, token, promptPath, conversationID string,
) {
	t.Helper()
	historyResponse := doConversationClientRequest(t, client, baseURL, token,
		http.MethodGet, promptPath, "", "", "")
	if historyResponse.StatusCode != http.StatusOK {
		t.Fatalf("client A history status=%d body=%q", historyResponse.StatusCode, readConversationClientBody(t, historyResponse))
	}
	var history model.ConversationPromptPage
	if err := json.NewDecoder(historyResponse.Body).Decode(&history); err != nil ||
		history.ConversationID != conversationID || len(history.Prompts) != 1 ||
		history.Prompts[0].Content != "prompt sent from client B" {
		t.Fatalf("client A history=%#v decode=%v", history, err)
	}
	_ = historyResponse.Body.Close()

	changesResponse := doConversationClientRequest(t, client, baseURL, token,
		http.MethodGet, conversationChangesPath+"?after_cursor=1&limit=50", "", "", "")
	if changesResponse.StatusCode != http.StatusOK {
		t.Fatalf("client A changes status=%d body=%q", changesResponse.StatusCode, readConversationClientBody(t, changesResponse))
	}
	var changes model.OwnedConversationChangePage
	if err := json.NewDecoder(changesResponse.Body).Decode(&changes); err != nil || len(changes.Changes) != 1 ||
		changes.Changes[0].ConversationID != conversationID || changes.Changes[0].Kind != "prompt_appended" {
		t.Fatalf("client A changes=%#v decode=%v", changes, err)
	}
	_ = changesResponse.Body.Close()
}

func tokenForIndependentClient(identity *conversationTestIdentity, scopes, clientID string) string {
	return tokenForPrincipal(identity, "account-42", scopes, clientID)
}

func tokenForPrincipal(identity *conversationTestIdentity, subject, scopes, clientID string) string {
	return tokenForPrincipalWithTTL(identity, subject, scopes, clientID, 5*time.Minute)
}

func tokenForPrincipalWithTTL(identity *conversationTestIdentity, subject, scopes, clientID string, ttl time.Duration) string {
	header, _ := json.Marshal(map[string]string{"typ": "at+jwt", "alg": "EdDSA", "kid": identity.keyID})
	claims, _ := json.Marshal(map[string]any{
		"iss": identity.issuer, "sub": subject, "aud": "forge-api",
		"exp": time.Now().Add(ttl).Unix(), "iat": time.Now().Add(-time.Minute).Unix(),
		"tenant_id": "tenant-slate", "scope": scopes, "jti": clientID,
	})
	input := base64URL(header) + "." + base64URL(claims)
	signature := ed25519Sign(identity, []byte(input))
	return input + "." + base64URL(signature)
}

func doConversationClientRequest(
	t *testing.T,
	client *http.Client,
	baseURL, token, method, target, contentType, idempotencyKey, body string,
) *http.Response {
	t.Helper()
	request, err := http.NewRequest(method, baseURL+target, strings.NewReader(body))
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Authorization", "Bearer "+token)
	if contentType != "" {
		request.Header.Set("Content-Type", contentType)
	}
	if idempotencyKey != "" {
		request.Header.Set("Idempotency-Key", idempotencyKey)
	}
	response, err := client.Do(request)
	if err != nil {
		t.Fatal(err)
	}
	return response
}

func readConversationClientBody(t *testing.T, response *http.Response) string {
	t.Helper()
	defer response.Body.Close()
	body, err := io.ReadAll(response.Body)
	if err != nil {
		t.Fatal(err)
	}
	return string(body)
}

func base64URL(value []byte) string {
	return base64.RawURLEncoding.EncodeToString(value)
}

func ed25519Sign(identity *conversationTestIdentity, value []byte) []byte {
	return ed25519.Sign(identity.key, value)
}
