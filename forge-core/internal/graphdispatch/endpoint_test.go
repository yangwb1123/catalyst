package graphdispatch

import "testing"

func TestEndpointGrammarAcceptsCanonicalHTTPSSubset(t *testing.T) {
	endpoints := []string{
		"https://api.openai.com/v1/responses",
		"https://api.example",
		"https://localhost/",
		"https://127.0.0.1/v1/responses",
		"https://api.example:8443/v1-beta_1/~models.json",
		"https://api.example//v1/responses",
	}
	for _, endpoint := range endpoints {
		t.Run(endpoint, func(t *testing.T) {
			if !validEndpoint(endpoint) {
				t.Fatalf("validEndpoint rejected %q", endpoint)
			}
		})
	}
}

func TestEndpointGrammarRejectsNormalizationAndAuthorityAmbiguity(t *testing.T) {
	tests := []struct {
		name     string
		endpoint string
	}{
		{"http scheme", "http://api.example/v1/responses"},
		{"uppercase scheme", "HTTPS://api.example/v1/responses"},
		{"userinfo", "https://user@api.example/v1/responses"},
		{"password", "https://user:secret@api.example/v1/responses"},
		{"query", "https://api.example/v1/responses?key=value"},
		{"fragment", "https://api.example/v1/responses#fragment"},
		{"dot segment", "https://api.example/v1/../responses"},
		{"single dot segment", "https://api.example/v1/./responses"},
		{"encoded dot segment", "https://api.example/v1/%2e/responses"},
		{"encoded double dot segment", "https://api.example/v1/%2E%2E/responses"},
		{"other percent encoding", "https://api.example/v1/%72esponses"},
		{"uppercase dns host", "https://API.Example/v1/responses"},
		{"unicode dns host", "https://café.example/v1/responses"},
		{"explicit default port", "https://api.example:443/v1/responses"},
		{"zero-padded port", "https://api.example:08443/v1/responses"},
		{"zero port", "https://api.example:0/v1/responses"},
		{"high port", "https://api.example:65536/v1/responses"},
		{"numeric host shorthand", "https://127.1/v1/responses"},
		{"zero-padded ipv4", "https://127.000.000.001/v1/responses"},
		{"numeric final dns label", "https://api.1/v1/responses"},
		{"ipv6 outside subset", "https://[::1]/v1/responses"},
		{"trailing dns dot", "https://api.example./v1/responses"},
		{"empty dns label", "https://api..example/v1/responses"},
		{"leading label hyphen", "https://-api.example/v1/responses"},
		{"trailing label hyphen", "https://api-.example/v1/responses"},
		{"dns underscore", "https://api_internal.example/v1/responses"},
		{"backslash", `https://api.example\v1\responses`},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if validEndpoint(test.endpoint) {
				t.Fatalf("validEndpoint accepted %q", test.endpoint)
			}
		})
	}
}
