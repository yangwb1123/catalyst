package appserver

import (
	"net/http"
	"net/http/httptest"
	"strconv"
	"testing"
)

var testHealthBody = []byte("{\"api_version\":\"forgeos.app-server/v1\",\"commit\":\"abc123\",\"service\":\"forge-server\",\"status\":\"ok\",\"version\":\"v0.1.0\"}\n")

func testRoutes(t *testing.T) http.Handler {
	t.Helper()
	return testRoutesWithLimit(t, maxInFlightRequests)
}

func testRoutesWithLimit(t *testing.T, maximum int) http.Handler {
	t.Helper()
	routes, err := newRoutesWithLimit(
		BuildInfo{Version: "v0.1.0", Commit: "abc123"}, "127.0.0.1:7467", maximum)
	if err != nil {
		t.Fatal(err)
	}
	return routes
}

func TestRoutesExposeOnlyExactVersionedHealthContract(t *testing.T) {
	tests := []struct {
		name, method, target, host, allow string
		status                            int
		body                              []byte
	}{
		{"health", http.MethodGet, HealthPath, "", "", http.StatusOK, testHealthBody},
		{"head", http.MethodHead, HealthPath, "", "", http.StatusOK, nil},
		{"missing", http.MethodGet, "/api/v1/missing", "", "", http.StatusNotFound, notFoundBody},
		{"mutation", http.MethodPost, HealthPath, "", "GET, HEAD", http.StatusMethodNotAllowed, methodBody},
		{"foreign-host", http.MethodGet, HealthPath, "attacker.example", "", http.StatusMisdirectedRequest, invalidAuthorityBody},
		{"encoded-separator", http.MethodGet, "/api%2Fv1%2Fhealth", "", "", http.StatusNotFound, notFoundBody},
		{"encoded-character", http.MethodGet, "/api/v1/%68ealth", "", "", http.StatusNotFound, notFoundBody},
		{"empty-query-alias", http.MethodGet, HealthPath + "?", "", "", http.StatusNotFound, notFoundBody},
		{"query-alias", http.MethodGet, HealthPath + "?probe=1", "", "", http.StatusNotFound, notFoundBody},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			response := recordRequest(testRoutes(t), test.method, test.target, test.host)
			if response.Code != test.status || response.Body.String() != string(test.body) {
				t.Fatalf("response = %d %q", response.Code, response.Body.String())
			}
			length := len(test.body)
			if test.method == http.MethodHead {
				length = len(testHealthBody)
			}
			assertContractHeaders(t, response.Header(), length, test.allow)
		})
	}
}

func TestRequestLimitReturnsExactVersionedBusyContract(t *testing.T) {
	response := recordRequest(testRoutesWithLimit(t, 0), http.MethodGet, HealthPath, "")
	if response.Code != http.StatusServiceUnavailable || response.Body.String() != string(overloadedBody) {
		t.Fatalf("busy response = %d %q", response.Code, response.Body.String())
	}
	assertContractHeaders(t, response.Header(), len(overloadedBody), "")
}

func TestRequestLimitRunsExactRoutePreflightBeforeBusyResponse(t *testing.T) {
	handler := testRoutesWithLimit(t, 0)
	tests := []struct {
		method, target, host, allow string
		status                      int
		body                        []byte
	}{
		{http.MethodGet, HealthPath, "attacker.example", "", http.StatusMisdirectedRequest, invalidAuthorityBody},
		{http.MethodGet, "/api/v1/missing", "", "", http.StatusNotFound, notFoundBody},
		{http.MethodPost, HealthPath, "", "GET, HEAD", http.StatusMethodNotAllowed, methodBody},
	}
	for _, test := range tests {
		response := recordRequest(handler, test.method, test.target, test.host)
		if response.Code != test.status || response.Body.String() != string(test.body) {
			t.Fatalf("preflight response = %d %q", response.Code, response.Body.String())
		}
		assertContractHeaders(t, response.Header(), len(test.body), test.allow)
	}
}

func recordRequest(handler http.Handler, method, target, host string) *httptest.ResponseRecorder {
	request := httptest.NewRequest(method, "http://127.0.0.1:7467"+target, nil)
	if host != "" {
		request.Host = host
	}
	recorder := httptest.NewRecorder()
	handler.ServeHTTP(recorder, request)
	return recorder
}

func assertContractHeaders(t *testing.T, header http.Header, contentLength int, allow string) {
	t.Helper()
	want := map[string]string{
		"Cache-Control":                "no-store",
		"Content-Length":               strconv.Itoa(contentLength),
		"Content-Security-Policy":      "default-src 'none'; frame-ancestors 'none'",
		"Content-Type":                 "application/json; charset=utf-8",
		"Cross-Origin-Resource-Policy": "same-origin",
		"Referrer-Policy":              "no-referrer",
		"X-Content-Type-Options":       "nosniff",
		"Allow":                        allow,
		"Access-Control-Allow-Origin":  "",
	}
	for name, value := range want {
		if got := header.Get(name); got != value {
			t.Errorf("%s = %q, want %q", name, got, value)
		}
	}
}

func TestRoutesRejectInvalidBuildIdentity(t *testing.T) {
	if _, err := newRoutes(BuildInfo{}, "127.0.0.1:7467"); err == nil {
		t.Fatal("invalid build identity succeeded")
	}
}

func TestRoutesRejectNonLoopbackAuthority(t *testing.T) {
	if _, err := newRoutes(BuildInfo{Version: "dev"}, "example.com:7467"); err == nil {
		t.Fatal("non-loopback route authority succeeded")
	}
}
