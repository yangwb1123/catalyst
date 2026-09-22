//go:build linux && !android

package appserver

import (
	"context"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestRunConfiguredServerKeepsObservationCandidatesClosed(t *testing.T) {
	issuer, ssoClient, _, token, closeSnaplink := startSnaplinkForgeTestIssuer(t)
	t.Cleanup(closeSnaplink)

	root := privateTestParent(t)
	stateDir := filepath.Join(root, "app-state")
	runtimeStateDir := filepath.Join(root, "runtime-state")
	if err := os.Mkdir(runtimeStateDir, 0o700); err != nil {
		t.Fatal(err)
	}
	runtimeExecutable := filepath.Join(root, "forge-runtime")
	if err := os.WriteFile(runtimeExecutable, []byte("test runtime"), 0o700); err != nil {
		t.Fatal(err)
	}

	config := Config{
		ListenAddress:       "127.0.0.1:0",
		StateDir:            stateDir,
		Build:               BuildInfo{Version: "test"},
		RuntimeExecutable:   runtimeExecutable,
		RuntimeStateDir:     runtimeStateDir,
		SnaplinkIssuer:      issuer,
		SnaplinkAudience:    snaplinkForgeTestAudience,
		SnaplinkJWKSURL:     issuer + "/.well-known/jwks.json",
		ExpectedTenantID:    snaplinkForgeTestTenant,
		ExpectedSubjectID:   snaplinkForgeTestUser,
		JWKSHTTPClient:      ssoClient,
		JWKSRefreshInterval: 24 * time.Hour,
	}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	readyChannel := make(chan Ready, 1)
	result := make(chan error, 1)
	resultConsumed := false
	go func() {
		result <- Run(ctx, config, func(ready Ready) error {
			readyChannel <- ready
			return nil
		})
	}()
	var ready Ready
	select {
	case ready = <-readyChannel:
	case err := <-result:
		resultConsumed = true
		t.Fatalf("configured server failed before readiness: %v", err)
	case <-time.After(5 * time.Second):
		t.Fatal("configured server did not announce readiness")
	}
	client := &http.Client{Timeout: 5 * time.Second}
	t.Cleanup(func() {
		cancel()
		if resultConsumed {
			return
		}
		if err := waitResult(t, result); err != nil {
			t.Error(err)
		}
	})

	tests := []struct {
		name, method, path, body, contentType string
	}{
		{name: "placement preview", method: http.MethodPost, path: devicePlacementPreviewPath, body: `{}`, contentType: "application/json"},
		{name: "session device observation", method: http.MethodPost, path: "/api/v1/conversations/conversation-1/runs/run-1/device-observation/preview", body: `{}`, contentType: "application/json"},
		{name: "runner receipt observation", method: http.MethodPost, path: "/api/v1/conversations/conversation-1/runs/run-1/runner-receipt-observation/preview", body: `{}`, contentType: "application/json"},
		{name: "local Runner preview", method: http.MethodPost, path: "/api/v1/conversations/conversation-1/run-intents/intent-1/execution-readiness-preview", body: `{}`, contentType: "application/json"},
		{name: "Run Attempt lease dispatch preflight", method: http.MethodPost, path: "/api/v1/conversations/conversation-1/runs/run-1/attempt-lease-dispatch-preflight/preview", body: `{}`, contentType: "application/json"},
		{name: "Run observed", method: http.MethodGet, path: "/api/v1/conversations/conversation-1/runs/run-1/observation"},
		{name: "execution consent preview", method: http.MethodGet, path: "/api/v1/conversations/conversation-1/execution-consents"},
		{name: "execution consent grant", method: http.MethodPost, path: "/api/v1/conversations/conversation-1/execution-consents", body: `{}`, contentType: "application/json"},
		{name: "pending run intent list", method: http.MethodGet, path: "/api/v1/conversations/conversation-1/run-intents"},
		{name: "pending run intent submit", method: http.MethodPost, path: "/api/v1/conversations/conversation-1/run-intents", body: `{}`, contentType: "application/json"},
		{name: "pending run intent timeline", method: http.MethodGet, path: "/api/v1/conversations/conversation-1/run-intents/intent-1/timeline"},
		{name: "execution consent revoke", method: http.MethodDelete, path: "/api/v1/execution-consents/grant-1"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			request, err := http.NewRequest(test.method, ready.Listen+test.path, strings.NewReader(test.body))
			if err != nil {
				t.Fatal(err)
			}
			request.Header.Set("Authorization", "Bearer "+token)
			if test.contentType != "" {
				request.Header.Set("Content-Type", test.contentType)
			}
			response, err := client.Do(request)
			if err != nil {
				t.Fatal(err)
			}
			defer response.Body.Close()
			body, err := io.ReadAll(response.Body)
			if err != nil {
				t.Fatal(err)
			}
			if response.StatusCode != http.StatusNotFound || string(body) != string(notFoundBody) {
				t.Fatalf("configured production route status=%d body=%q", response.StatusCode, body)
			}
			assertContractHeaders(t, response.Header, len(notFoundBody), "")
		})
	}
}
