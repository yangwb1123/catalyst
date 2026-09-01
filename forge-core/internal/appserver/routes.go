package appserver

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strconv"
)

const HealthPath = "/api/v1/health"

var (
	invalidAuthorityBody = []byte("{\"api_version\":\"forgeos.app-server/v1\",\"code\":\"invalid_authority\",\"message\":\"request authority rejected\"}\n")
	notFoundBody         = []byte("{\"api_version\":\"forgeos.app-server/v1\",\"code\":\"not_found\",\"message\":\"route not found\"}\n")
	methodBody           = []byte("{\"api_version\":\"forgeos.app-server/v1\",\"code\":\"method_not_allowed\",\"message\":\"method not allowed\"}\n")
	overloadedBody       = []byte("{\"api_version\":\"forgeos.app-server/v1\",\"code\":\"server_busy\",\"message\":\"server request limit reached\"}\n")
)

type healthResponse struct {
	APIVersion string `json:"api_version"`
	Commit     string `json:"commit"`
	Service    string `json:"service"`
	Status     string `json:"status"`
	Version    string `json:"version"`
}

type routes struct {
	authority string
	health    http.Handler
}

// newRoutes builds the package-private read-only surface served only by Run.
func newRoutes(build BuildInfo, authority string) (http.Handler, error) {
	return newRoutesWithLimit(build, authority, maxInFlightRequests)
}

func newRoutesWithLimit(build BuildInfo, authority string, maximum int) (http.Handler, error) {
	if err := build.validate(); err != nil {
		return nil, err
	}
	if err := validateListenAddress(authority); err != nil {
		return nil, fmt.Errorf("route authority: %w", err)
	}
	body, err := json.Marshal(healthResponse{
		APIVersion: APIVersion, Commit: build.Commit, Service: "forge-server",
		Status: "ok", Version: build.Version,
	})
	if err != nil {
		return nil, fmt.Errorf("encode health response: %w", err)
	}
	healthBody := append(body, '\n')
	health := http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		writeJSON(writer, request, http.StatusOK, healthBody)
	})
	return routes{authority: authority, health: limitRequests(health, maximum)}, nil
}

func (r routes) ServeHTTP(writer http.ResponseWriter, request *http.Request) {
	if request.Host != r.authority {
		writeJSON(writer, request, http.StatusMisdirectedRequest, invalidAuthorityBody)
		return
	}
	if request.URL.EscapedPath() != HealthPath || request.URL.ForceQuery || request.URL.RawQuery != "" {
		writeJSON(writer, request, http.StatusNotFound, notFoundBody)
		return
	}
	if request.Method != http.MethodGet && request.Method != http.MethodHead {
		writer.Header().Set("Allow", "GET, HEAD")
		writeJSON(writer, request, http.StatusMethodNotAllowed, methodBody)
		return
	}
	r.health.ServeHTTP(writer, request)
}

func writeJSON(writer http.ResponseWriter, request *http.Request, status int, body []byte) {
	header := writer.Header()
	header.Set("Cache-Control", "no-store")
	header.Set("Content-Length", strconv.Itoa(len(body)))
	header.Set("Content-Security-Policy", "default-src 'none'; frame-ancestors 'none'")
	header.Set("Content-Type", "application/json; charset=utf-8")
	header.Set("Cross-Origin-Resource-Policy", "same-origin")
	header.Set("Referrer-Policy", "no-referrer")
	header.Set("X-Content-Type-Options", "nosniff")
	writer.WriteHeader(status)
	if request.Method != http.MethodHead {
		_, _ = writer.Write(body)
	}
}
