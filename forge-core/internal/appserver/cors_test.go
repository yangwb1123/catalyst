package appserver

import (
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestBrowserOriginsAllowOnlyExactBearerAPIRequests(t *testing.T) {
	called := false
	handler := allowBrowserOrigins(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		called = true
		w.WriteHeader(http.StatusNoContent)
	}), []string{"https://console.example"})

	allowed := httptest.NewRequest(http.MethodGet, "https://forge.example/api/v1/conversations", nil)
	allowed.Header.Set("Origin", "https://console.example")
	allowedResponse := httptest.NewRecorder()
	handler.ServeHTTP(allowedResponse, allowed)
	if allowedResponse.Code != http.StatusNoContent || !called ||
		allowedResponse.Header().Get("Access-Control-Allow-Origin") != "https://console.example" {
		t.Fatalf("allowed origin response = %d %#v", allowedResponse.Code, allowedResponse.Header())
	}
	if allowedResponse.Header().Get("Access-Control-Allow-Credentials") != "" {
		t.Fatal("bearer-only API unexpectedly enables credentialed browser requests")
	}

	called = false
	denied := httptest.NewRequest(http.MethodGet, "https://forge.example/api/v1/conversations", nil)
	denied.Header.Set("Origin", "https://attacker.example")
	deniedResponse := httptest.NewRecorder()
	handler.ServeHTTP(deniedResponse, denied)
	if deniedResponse.Code != http.StatusForbidden || called || deniedResponse.Header().Get("Access-Control-Allow-Origin") != "" {
		t.Fatalf("disallowed origin response = %d %#v called=%v", deniedResponse.Code, deniedResponse.Header(), called)
	}
}

func TestBrowserOriginPreflightIsBoundedAndUnauthenticated(t *testing.T) {
	called := false
	handler := allowBrowserOrigins(http.HandlerFunc(func(http.ResponseWriter, *http.Request) { called = true }), []string{"https://console.example"})
	preflight := httptest.NewRequest(http.MethodOptions, "https://forge.example/api/v1/conversations", nil)
	preflight.Header.Set("Origin", "https://console.example")
	preflight.Header.Set("Access-Control-Request-Method", "POST")
	preflight.Header.Set("Access-Control-Request-Headers", "authorization,cache-control,content-type,idempotency-key")
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, preflight)
	if response.Code != http.StatusNoContent || called || response.Header().Get("Access-Control-Allow-Methods") != "GET, POST, OPTIONS" {
		t.Fatalf("preflight response = %d %#v called=%v", response.Code, response.Header(), called)
	}
	if response.Header().Get("Access-Control-Allow-Headers") != "Authorization, Cache-Control, Content-Type, Idempotency-Key" {
		t.Fatalf("preflight allowed headers = %q", response.Header().Get("Access-Control-Allow-Headers"))
	}

	for name, requestValues := range map[string][2]string{
		"execution_method": {"DELETE", "authorization"},
		"unknown_header":   {"POST", "authorization, x-debug-token"},
		"missing_method":   {"", "authorization"},
	} {
		bad := httptest.NewRequest(http.MethodOptions, "https://forge.example/api/v1/conversations", nil)
		bad.Header.Set("Origin", "https://console.example")
		if requestValues[0] != "" {
			bad.Header.Set("Access-Control-Request-Method", requestValues[0])
		}
		bad.Header.Set("Access-Control-Request-Headers", requestValues[1])
		badResponse := httptest.NewRecorder()
		handler.ServeHTTP(badResponse, bad)
		if badResponse.Code != http.StatusForbidden {
			t.Errorf("%s preflight status=%d", name, badResponse.Code)
		}
	}
}
