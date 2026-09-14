package appserver

import (
	"net/http"
	"strings"
)

var corsAllowedHeaders = map[string]struct{}{
	"authorization":   {},
	"cache-control":   {},
	"content-type":    {},
	"idempotency-key": {},
}

// allowBrowserOrigins grants CORS access to exact configured web origins.
// It never enables cookies; browser clients send explicit bearer tokens.
func allowBrowserOrigins(next http.Handler, origins []string) http.Handler {
	if next == nil || len(origins) == 0 {
		return next
	}
	allowed := make(map[string]struct{}, len(origins))
	for _, origin := range origins {
		allowed[origin] = struct{}{}
	}
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		values := r.Header.Values("Origin")
		if len(values) == 0 {
			next.ServeHTTP(w, r)
			return
		}
		if len(values) != 1 {
			writeJSON(w, r, http.StatusForbidden, notFoundBody)
			return
		}
		origin := values[0]
		if _, ok := allowed[origin]; !ok {
			writeJSON(w, r, http.StatusForbidden, notFoundBody)
			return
		}
		w.Header().Set("Access-Control-Allow-Origin", origin)
		w.Header().Add("Vary", "Origin")
		if r.Method != http.MethodOptions {
			next.ServeHTTP(w, r)
			return
		}
		if !validPreflightMethod(r) || !validPreflightHeaders(r) {
			writeJSON(w, r, http.StatusForbidden, notFoundBody)
			return
		}
		w.Header().Set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Authorization, Cache-Control, Content-Type, Idempotency-Key")
		w.Header().Set("Access-Control-Max-Age", "600")
		w.Header().Add("Vary", "Access-Control-Request-Method")
		w.Header().Add("Vary", "Access-Control-Request-Headers")
		w.WriteHeader(http.StatusNoContent)
	})
}

func validPreflightMethod(r *http.Request) bool {
	values := r.Header.Values("Access-Control-Request-Method")
	if len(values) != 1 {
		return false
	}
	method := strings.TrimSpace(values[0])
	return method == http.MethodGet || method == http.MethodPost
}

func validPreflightHeaders(r *http.Request) bool {
	values := r.Header.Values("Access-Control-Request-Headers")
	if len(values) == 0 {
		return true
	}
	if len(values) != 1 {
		return false
	}
	for _, header := range strings.Split(values[0], ",") {
		name := strings.ToLower(strings.TrimSpace(header))
		if name == "" {
			return false
		}
		if _, ok := corsAllowedHeaders[name]; !ok {
			return false
		}
	}
	return true
}
