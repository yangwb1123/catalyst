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

// TestRunConfiguredServerKeepsDeviceAndClientInstanceCandidatesClosed checks
// the actual Run -> outer routes boundary. The focused candidate tests mount
// injected sources on private muxes; this regression proves those paths cannot
// become reachable merely because a production server has a configured
// Runtime, Snaplink issuer, and listener.
func TestRunConfiguredServerKeepsDeviceAndClientInstanceCandidatesClosed(t *testing.T) {
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
		name, method, path string
	}{
		{name: "enrollment", method: http.MethodPost, path: "/api/v1/devices/enrollments"},
		{name: "heartbeat", method: http.MethodPost, path: "/api/v1/devices/device-a/heartbeats"},
		{name: "inventory v1", method: http.MethodGet, path: deviceInventoryReadCandidatePath},
		{name: "inventory v1 trailing slash", method: http.MethodGet, path: deviceInventoryReadCandidatePath + "/"},
		{name: "inventory v2", method: http.MethodGet, path: deviceInventoryReadCandidateV2Path},
		{name: "client instance session view", method: http.MethodGet, path: clientInstanceSessionViewCandidatePath},
		{name: "client instance resource view", method: http.MethodGet, path: clientInstanceResourceViewCandidatePath},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			request, err := http.NewRequest(test.method, ready.Listen+test.path, strings.NewReader(""))
			if err != nil {
				t.Fatal(err)
			}
			request.Header.Set("Authorization", "Bearer "+token)
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
