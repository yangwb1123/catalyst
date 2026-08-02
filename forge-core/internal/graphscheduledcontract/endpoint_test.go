package graphscheduledcontract

import "testing"

func TestEndpointGrammarMatchesFrozenContractBoundary(t *testing.T) {
	valid := []string{
		"https://api.example", "https://api.example/v1/responses",
		"https://127.0.0.1:8443/root", "https://a-b.example/path_OK~",
	}
	invalid := []string{
		"http://api.example", "https://API.example/v1", "https://api.example:443/v1",
		"https://user@api.example/v1", "https://api.example/v1?x=1",
		"https://api.example/v1#x", "https://api.example/v1/../x",
		"https://api.example/v1/%2e", "https://[::1]/v1", "https://01.2.3.4/v1",
	}
	for _, endpoint := range valid {
		if !validEndpoint(endpoint) {
			t.Errorf("valid endpoint rejected: %q", endpoint)
		}
	}
	for _, endpoint := range invalid {
		if validEndpoint(endpoint) {
			t.Errorf("invalid endpoint accepted: %q", endpoint)
		}
	}
}
