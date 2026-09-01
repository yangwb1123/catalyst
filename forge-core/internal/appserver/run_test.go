//go:build linux && !android

package appserver

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

type testServer struct {
	ready  Ready
	cancel context.CancelFunc
	result <-chan error
}

func startTestServer(t *testing.T, stateDir string) testServer {
	t.Helper()
	ctx, cancel := context.WithCancel(context.Background())
	readyChannel := make(chan Ready, 1)
	result := make(chan error, 1)
	config := Config{
		ListenAddress: "127.0.0.1:0", StateDir: stateDir,
		Build: BuildInfo{Version: "dev"},
	}
	go func() {
		result <- Run(ctx, config, func(ready Ready) error {
			readyChannel <- ready
			return nil
		})
	}()
	return testServer{ready: waitReady(t, readyChannel), cancel: cancel, result: result}
}

func waitReady(t *testing.T, ready <-chan Ready) Ready {
	t.Helper()
	select {
	case value := <-ready:
		return value
	case <-time.After(5 * time.Second):
		t.Fatal("server did not announce readiness")
		return Ready{}
	}
}

func waitResult(t *testing.T, result <-chan error) error {
	t.Helper()
	select {
	case err := <-result:
		return err
	case <-time.After(5 * time.Second):
		t.Fatal("server did not stop")
		return nil
	}
}

func (server testServer) stop(t *testing.T) {
	t.Helper()
	server.cancel()
	if err := waitResult(t, server.result); err != nil {
		t.Fatal(err)
	}
}

func localHTTPClient() *http.Client {
	return &http.Client{
		Transport: &http.Transport{Proxy: nil},
		Timeout:   2 * time.Second,
	}
}

func TestRunServesHealthAndStopsGracefully(t *testing.T) {
	server := startTestServer(t, filepath.Join(privateTestParent(t), "state"))
	response, err := localHTTPClient().Get(server.ready.Listen + HealthPath)
	if err != nil {
		server.stop(t)
		t.Fatal(err)
	}
	body, readErr := io.ReadAll(response.Body)
	closeErr := response.Body.Close()
	if response.StatusCode != http.StatusOK || readErr != nil || closeErr != nil {
		server.stop(t)
		t.Fatalf("health request = %d, %v, %v", response.StatusCode, readErr, closeErr)
	}
	want := []byte("{\"api_version\":\"forgeos.app-server/v1\",\"commit\":\"\",\"service\":\"forge-server\",\"status\":\"ok\",\"version\":\"dev\"}\n")
	if string(body) != string(want) {
		server.stop(t)
		t.Fatalf("wire health body = %q", body)
	}
	assertContractHeaders(t, response.Header, len(want), "")
	server.stop(t)
}

func TestRunRejectsSecondServerForSameStateDirectory(t *testing.T) {
	stateDir := filepath.Join(privateTestParent(t), "state")
	server := startTestServer(t, stateDir)
	config := Config{
		ListenAddress: "127.0.0.1:0", StateDir: stateDir,
		Build: BuildInfo{Version: "dev"},
	}
	err := Run(context.Background(), config, func(Ready) error { return nil })
	server.stop(t)
	if !errors.Is(err, ErrAlreadyRunning) {
		t.Fatalf("second server error = %v", err)
	}
}

func TestRunRejectsNilContextBeforeStateMutation(t *testing.T) {
	config := testConfig(t)
	err := Run(nil, config, nil)
	if err == nil {
		t.Fatal("nil context succeeded")
	}
}

func TestRunRejectsNilAnnouncerBeforeStateMutation(t *testing.T) {
	stateDir := filepath.Join(privateTestParent(t), "state")
	err := Run(context.Background(), serverConfig(stateDir), nil)
	if !errors.Is(err, errAnnouncerRequired) {
		t.Fatalf("nil announcer error = %v", err)
	}
	if _, err := os.Lstat(stateDir); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("nil announcer mutated state: %v", err)
	}
}

func TestRunRejectsPreCancelledContextBeforeStateMutation(t *testing.T) {
	stateDir := filepath.Join(privateTestParent(t), "state")
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	err := Run(ctx, serverConfig(stateDir), func(Ready) error { return nil })
	if !errors.Is(err, context.Canceled) {
		t.Fatalf("pre-cancelled run error = %v", err)
	}
	if _, err := os.Lstat(stateDir); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("pre-cancelled run mutated state: %v", err)
	}
}

func TestRunRejectsNoncanonicalStatePathBeforeMutation(t *testing.T) {
	parent := privateTestParent(t)
	outside := filepath.Join(parent, "outside")
	child := filepath.Join(outside, "child")
	if err := os.MkdirAll(child, 0o700); err != nil {
		t.Fatal(err)
	}
	alias := filepath.Join(parent, "alias")
	if err := os.Symlink(child, alias); err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		path    string
		mutated []string
	}{
		{parent + "/alias/../state", []string{filepath.Join(parent, "state"), filepath.Join(outside, "state")}},
		{parent + "/missing/../state", []string{filepath.Join(parent, "state")}},
	}
	for _, test := range tests {
		if err := Run(context.Background(), serverConfig(test.path), func(Ready) error { return nil }); err == nil {
			t.Fatalf("noncanonical state path %q succeeded", test.path)
		}
		for _, path := range test.mutated {
			if _, err := os.Lstat(path); !errors.Is(err, os.ErrNotExist) {
				t.Fatalf("noncanonical path mutated %s: %v", path, err)
			}
		}
	}
}

func TestRunDoesNotServeUntilAnnouncementCompletes(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	readyChannel := make(chan Ready, 1)
	release := make(chan struct{})
	result := make(chan error, 1)
	stateDir := filepath.Join(privateTestParent(t), "state")
	go func() {
		result <- Run(ctx, serverConfig(stateDir), func(ready Ready) error {
			readyChannel <- ready
			<-release
			return nil
		})
	}()
	ready := waitReady(t, readyChannel)
	request := make(chan *http.Response, 1)
	go func() {
		response, _ := localHTTPClient().Get(ready.Listen + HealthPath)
		request <- response
	}()
	assertNoResponse(t, request)
	close(release)
	response := waitHTTPResponse(t, request)
	_ = response.Body.Close()
	if response.StatusCode != http.StatusOK {
		t.Fatalf("health after announcement = %d", response.StatusCode)
	}
	cancel()
	if err := waitResult(t, result); err != nil {
		t.Fatal(err)
	}
}

func TestRunCancelsWhileAnnouncementIsBlocked(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	started := make(chan Ready, 1)
	release := make(chan struct{})
	result := make(chan error, 1)
	stateDir := filepath.Join(privateTestParent(t), "state")
	go func() {
		result <- Run(ctx, serverConfig(stateDir), func(ready Ready) error {
			started <- ready
			<-release
			return nil
		})
	}()
	ready := waitReady(t, started)
	cancel()
	if err := waitResult(t, result); err != nil {
		t.Fatal(err)
	}
	close(release)
	if _, err := localHTTPClient().Get(ready.Listen + HealthPath); err == nil {
		t.Fatal("cancelled startup left a reachable server")
	}
}

func TestRunAnnouncementFailureNeverServes(t *testing.T) {
	want := errors.New("output failed")
	readyChannel := make(chan Ready, 1)
	err := Run(context.Background(), serverConfig(filepath.Join(privateTestParent(t), "state")), func(ready Ready) error {
		readyChannel <- ready
		return want
	})
	ready := <-readyChannel
	if !errors.Is(err, want) {
		t.Fatalf("announcement error = %v", err)
	}
	if _, requestErr := localHTTPClient().Get(ready.Listen + HealthPath); requestErr == nil {
		t.Fatal("announcement failure left a reachable server")
	}
}

func TestAnnouncementHasBoundedTimeout(t *testing.T) {
	release := make(chan struct{})
	err := awaitAnnouncement(context.Background(), func(Ready) error {
		<-release
		return nil
	}, Ready{}, 20*time.Millisecond)
	close(release)
	if err == nil || !strings.Contains(err.Error(), "timed out") {
		t.Fatalf("blocked announcement error = %v", err)
	}
}

func TestServerWireRejectsNoncanonicalAndGeneralRoutes(t *testing.T) {
	server := startTestServer(t, filepath.Join(privateTestParent(t), "state"))
	defer server.stop(t)
	authority := readyAuthority(t, server.ready)
	tests := []struct {
		name, requestLine, method string
	}{
		{"general-options", "OPTIONS * HTTP/1.1", http.MethodOptions},
		{"encoded-separator", "GET /api%2Fv1%2Fhealth HTTP/1.1", http.MethodGet},
		{"encoded-character", "GET /api/v1/%68ealth HTTP/1.1", http.MethodGet},
		{"query-alias", "GET /api/v1/health?probe=1 HTTP/1.1", http.MethodGet},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			response := rawHTTPResponse(t, authority, test.requestLine, test.method)
			defer func() { _ = response.Body.Close() }()
			body, err := io.ReadAll(response.Body)
			if err != nil || response.StatusCode != http.StatusNotFound || string(body) != string(notFoundBody) {
				t.Fatalf("raw rejection = %d %q, %v", response.StatusCode, body, err)
			}
			assertContractHeaders(t, response.Header, len(notFoundBody), "")
		})
	}
}

func rawHTTPResponse(t *testing.T, authority, requestLine, method string) *http.Response {
	t.Helper()
	connection, err := net.DialTimeout("tcp", authority, time.Second)
	if err != nil {
		t.Fatal(err)
	}
	_, _ = fmt.Fprintf(connection, "%s\r\nHost: %s\r\nConnection: close\r\n\r\n", requestLine, authority)
	response, err := http.ReadResponse(bufio.NewReader(connection), &http.Request{Method: method})
	if err != nil {
		_ = connection.Close()
		t.Fatal(err)
	}
	return response
}

func TestServerRejectsForeignHost(t *testing.T) {
	server := startTestServer(t, filepath.Join(privateTestParent(t), "state"))
	defer server.stop(t)
	request, err := http.NewRequest(http.MethodGet, server.ready.Listen+HealthPath, nil)
	if err != nil {
		t.Fatal(err)
	}
	request.Host = "attacker.example"
	response, err := localHTTPClient().Do(request)
	if err != nil {
		t.Fatal(err)
	}
	_ = response.Body.Close()
	if response.StatusCode != http.StatusMisdirectedRequest {
		t.Fatalf("foreign Host status = %d", response.StatusCode)
	}
}

func serverConfig(stateDir string) Config {
	return Config{
		ListenAddress: "127.0.0.1:0", StateDir: stateDir,
		Build: BuildInfo{Version: "dev"},
	}
}

func assertNoResponse(t *testing.T, response <-chan *http.Response) {
	t.Helper()
	select {
	case got := <-response:
		if got != nil {
			_ = got.Body.Close()
		}
		t.Fatal("server responded before startup announcement completed")
	case <-time.After(100 * time.Millisecond):
	}
}

func waitHTTPResponse(t *testing.T, response <-chan *http.Response) *http.Response {
	t.Helper()
	select {
	case got := <-response:
		if got == nil {
			t.Fatal("health request failed")
		}
		return got
	case <-time.After(3 * time.Second):
		t.Fatal("health response timed out")
		return nil
	}
}

func readyAuthority(t *testing.T, ready Ready) string {
	t.Helper()
	parsed, err := url.Parse(ready.Listen)
	if err != nil || parsed.Host == "" {
		t.Fatalf("ready listen URL = %q, %v", ready.Listen, err)
	}
	return parsed.Host
}
