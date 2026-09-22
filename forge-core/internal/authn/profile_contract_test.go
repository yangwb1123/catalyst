package authn

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"testing"
)

func TestSnaplinkProfileContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_SNAPLINK_PROFILE_FIXTURE")
	if path == "" {
		t.Skip("FORGE_SNAPLINK_PROFILE_FIXTURE is set by the cross-repository contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var fixture snaplinkProfileFixture
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode Snaplink profile fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("Snaplink profile fixture has trailing JSON: %v", err)
	}
	if fixture.SchemaVersion != "forge.snaplink-profile/v1" ||
		fixture.EvaluationMode != "configuration_parity_only" ||
		fixture.Issuer != "https://id.example" || fixture.Audience != "forge-api" ||
		fixture.Resource != fixture.Audience || len(fixture.ConversationScopes) != 2 ||
		fixture.ConversationScopes[0] != "forge:conversations:read" ||
		fixture.ConversationScopes[1] != "forge:conversations:write" {
		t.Fatalf("invalid Snaplink profile envelope: %#v", fixture)
	}
	if err := ValidateConfig(Config{Issuer: fixture.Issuer, Audience: fixture.Audience, JWKSURL: fixture.Issuer + "/jwks"}); err != nil {
		t.Fatalf("profile does not satisfy Forge resource-server validation: %v", err)
	}
	assertSnaplinkProfileClient(t, fixture.Clients["cli"], "forge-cli", []string{"urn:ietf:params:oauth:grant-type:device_code", "refresh_token"}, fixture.ConversationScopes)
	assertSnaplinkProfileClient(t, fixture.Clients["console"], "forge-console", []string{"authorization_code", "refresh_token"}, fixture.ConversationScopes)
	if fixture.Authority != (snaplinkProfileAuthority{}) {
		t.Fatalf("profile fixture must not claim live authority: %#v", fixture.Authority)
	}
}

type snaplinkProfileFixture struct {
	SchemaVersion      string                           `json:"schema_version"`
	EvaluationMode     string                           `json:"evaluation_mode"`
	Issuer             string                           `json:"issuer"`
	Audience           string                           `json:"audience"`
	Resource           string                           `json:"resource"`
	ConversationScopes []string                         `json:"conversation_scopes"`
	Clients            map[string]snaplinkProfileClient `json:"clients"`
	Authority          snaplinkProfileAuthority         `json:"authority"`
}

type snaplinkProfileClient struct {
	ClientID   string   `json:"client_id"`
	Public     bool     `json:"public"`
	GrantTypes []string `json:"grant_types"`
	Scopes     []string `json:"scopes"`
}

type snaplinkProfileAuthority struct {
	IssuerVerified            bool `json:"issuer_verified"`
	AudienceVerified          bool `json:"audience_verified"`
	ClientProvisioned         bool `json:"client_provisioned"`
	TokenIssued               bool `json:"token_issued"`
	DeviceAuthorized          bool `json:"device_authorized"`
	ConversationAccessGranted bool `json:"conversation_access_granted"`
}

func assertSnaplinkProfileClient(t *testing.T, client snaplinkProfileClient, id string, grants, scopes []string) {
	t.Helper()
	if client.ClientID != id || !client.Public || !equalStrings(client.GrantTypes, grants) || !equalStrings(client.Scopes, scopes) {
		t.Fatalf("invalid Snaplink client profile: %#v", client)
	}
}

func equalStrings(left, right []string) bool {
	if len(left) != len(right) {
		return false
	}
	for index := range left {
		if left[index] != right[index] {
			return false
		}
	}
	return true
}
