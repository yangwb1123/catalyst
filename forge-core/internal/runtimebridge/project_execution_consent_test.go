package runtimebridge

import (
	"context"
	"encoding/json"
	consentmodel "forgeos/forge-core/internal/runtimebridge/consentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"path/filepath"
	"strings"
	"testing"
)

func TestGrantProjectExecutionConsentUsesRuntimeV2AndExactOwnerScope(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "app-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	owner := consentTestOwner()
	var digest [32]byte
	for index := range digest {
		digest[index] = byte(index)
	}
	digestJSON, err := json.Marshal(digest)
	if err != nil {
		t.Fatal(err)
	}
	const expiry = uint64(200)
	result := `{"grant":{"grant_id":"grant-1","project_id":"project-1","profile_id":"profile-1","profile_sha256":` +
		string(digestJSON) + `,"granted_at_ms":100,"expires_at_ms":200},"replayed":true}`
	writeFake(t, executable, responseScriptCheckingRequest(database, result, writeProtocolVersion,
		`"api_version":"forgeos.runtime-bridge/v2"`, `"operation":"grant_project_execution_consent"`,
		`"project_id":"project-1"`, `"profile_id":"profile-1"`, `"profile_sha256":`+string(digestJSON),
		`"expires_at_ms":200`, `"idempotency_key":"grant-key"`,
		`"owner":{"issuer":"https://identity.example","subject":"account-42","tenant_id":"tenant-slate"}`))
	client := newConsentTestClient(t, executable, appState, runtimeState)
	grant, err := client.GrantProjectExecutionConsent(context.Background(), owner,
		"project-1", "profile-1", digest, expiry, "grant-key")
	if err != nil || !grant.Replayed || grant.Grant.GrantID != "grant-1" ||
		grant.Grant.ProjectID != "project-1" || grant.Grant.ExpiresAtMS != expiry {
		t.Fatalf("grant receipt=%#v error=%v", grant, err)
	}
}

func TestRevokeProjectExecutionConsentUsesRuntimeV2AndExactGrant(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "app-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	database := filepath.Join(runtimeState, "hub.sqlite3")
	result := `{"revocation":{"event_id":"event-1","grant_id":"grant-1","revoked_at_ms":300},"replayed":false}`
	writeFake(t, executable, responseScriptCheckingRequest(database, result, writeProtocolVersion,
		`"api_version":"forgeos.runtime-bridge/v2"`, `"operation":"revoke_project_execution_consent"`,
		`"grant_id":"grant-1"`, `"idempotency_key":"revoke-key"`,
		`"owner":{"issuer":"https://identity.example","subject":"account-42","tenant_id":"tenant-slate"}`))
	client := newConsentTestClient(t, executable, appState, runtimeState)
	revocation, err := client.RevokeProjectExecutionConsent(context.Background(), consentTestOwner(),
		"grant-1", "revoke-key")
	if err != nil || revocation.Replayed || revocation.Revocation.EventID != "event-1" ||
		revocation.Revocation.GrantID != "grant-1" || revocation.Revocation.RevokedAtMS != 300 {
		t.Fatalf("revocation receipt=%#v error=%v", revocation, err)
	}
}

func TestProjectExecutionConsentRequestValidationAndStableErrorCode(t *testing.T) {
	client := &Client{executable: "/path/that/must/not/be/run"}
	owner := consentTestOwner()
	digest := [32]byte{1}
	for _, test := range []struct {
		name string
		call func() error
	}{
		{"owner control", func() error {
			badOwner := owner
			badOwner.Subject = "bad\u0085owner"
			_, err := client.GrantProjectExecutionConsent(context.Background(), badOwner, "p1", "profile1", digest, 10, "key")
			return err
		}},
		{"project control", func() error {
			_, err := client.GrantProjectExecutionConsent(context.Background(), owner, "project\n1", "profile1", digest, 10, "key")
			return err
		}},
		{"profile blank", func() error {
			_, err := client.GrantProjectExecutionConsent(context.Background(), owner, "p1", " ", digest, 10, "key")
			return err
		}},
		{"expiry range", func() error {
			_, err := client.GrantProjectExecutionConsent(context.Background(), owner, "p1", "profile1", digest, maxSQLiteInteger+1, "key")
			return err
		}},
		{"grant key control", func() error {
			_, err := client.GrantProjectExecutionConsent(context.Background(), owner, "p1", "profile1", digest, 10, "bad\nkey")
			return err
		}},
		{"revoke key control", func() error {
			_, err := client.RevokeProjectExecutionConsent(context.Background(), owner, "grant1", "bad\u0085key")
			return err
		}},
		{"grant id blank", func() error {
			_, err := client.RevokeProjectExecutionConsent(context.Background(), owner, " ", "key")
			return err
		}},
	} {
		t.Run(test.name, func(t *testing.T) {
			err := test.call()
			if err == nil || err.(*Error).Code != "invalid_project_execution_consent_request" {
				t.Fatalf("invalid request error=%v", err)
			}
		})
	}
	if got := stableRuntimeErrorCode("invalid_project_execution_consent_request"); got != "invalid_project_execution_consent_request" {
		t.Fatalf("invalid consent error code not retained: %q", got)
	}
}

func TestProjectExecutionConsentResponseValidationRejectsConfusedOrExpandedDTOs(t *testing.T) {
	var digest [32]byte
	for index := range digest {
		digest[index] = byte(index)
	}
	digestJSON, err := json.Marshal(digest)
	if err != nil {
		t.Fatal(err)
	}
	validGrant := `{"grant":{"grant_id":"grant-1","project_id":"project-1","profile_id":"profile-1","profile_sha256":` +
		string(digestJSON) + `,"granted_at_ms":100,"expires_at_ms":200},"replayed":true}`
	var grant consentmodel.ProjectExecutionConsentGrantResult
	if err := decodeStrict([]byte(validGrant), &grant); err != nil ||
		!validProjectExecutionConsentGrant([]byte(validGrant), grant, "project-1", "profile-1", digest, 200) {
		t.Fatalf("valid grant receipt rejected: %#v, %v", grant, err)
	}
	shortDigest := strings.Replace(validGrant, string(digestJSON), "[0,1]", 1)
	wrongDigest := strings.Replace(validGrant, "[0,1,2", "[1,1,2", 1)
	for _, malformed := range []string{
		strings.Replace(validGrant, `"replayed":true`, `"replayed":true,"extra":1`, 1),
		strings.Replace(validGrant, `"expires_at_ms":200`, `"expires_at_ms":200,"owner":"leak"`, 1),
		strings.Replace(validGrant, `"project_id":"project-1"`, `"project_id":"project-2"`, 1),
		strings.Replace(validGrant, `"profile_id":"profile-1"`, `"profile_id":"profile-2"`, 1),
		shortDigest,
		wrongDigest,
		strings.Replace(validGrant, `"expires_at_ms":200`, `"expires_at_ms":201`, 1),
		strings.Replace(validGrant, `"granted_at_ms":100`, `"granted_at_ms":201`, 1),
		strings.Replace(validGrant, `,"replayed":true`, ``, 1),
		strings.Replace(validGrant, `"replayed":true`, `"replayed":"true"`, 1),
	} {
		var result consentmodel.ProjectExecutionConsentGrantResult
		if err := decodeStrict([]byte(malformed), &result); err == nil &&
			validProjectExecutionConsentGrant([]byte(malformed), result, "project-1", "profile-1", digest, 200) {
			t.Fatalf("malformed grant receipt accepted: %s", malformed)
		}
	}
	tooLateGrant := strings.Replace(validGrant, `"granted_at_ms":100`, `"granted_at_ms":9223372036854775808`, 1)
	var outOfRange consentmodel.ProjectExecutionConsentGrantResult
	if err := decodeStrict([]byte(tooLateGrant), &outOfRange); err == nil &&
		validProjectExecutionConsentGrant([]byte(tooLateGrant), outOfRange, "project-1", "profile-1", digest, 200) {
		t.Fatal("out-of-range grant timestamp accepted")
	}
	assertProjectExecutionConsentRevocationValidation(t)
}

func assertProjectExecutionConsentRevocationValidation(t *testing.T) {
	t.Helper()
	validRevocation := `{"revocation":{"event_id":"event-1","grant_id":"grant-1","revoked_at_ms":300},"replayed":false}`
	var revocation consentmodel.ProjectExecutionConsentRevocationResult
	if err := decodeStrict([]byte(validRevocation), &revocation); err != nil ||
		!validProjectExecutionConsentRevocation([]byte(validRevocation), revocation, "grant-1") {
		t.Fatalf("valid revocation receipt rejected: %#v, %v", revocation, err)
	}
	for _, malformed := range []string{
		strings.Replace(validRevocation, `"replayed":false`, `"replayed":false,"extra":1`, 1),
		strings.Replace(validRevocation, `"revoked_at_ms":300`, `"revoked_at_ms":300,"payload":"leak"`, 1),
		strings.Replace(validRevocation, `"grant_id":"grant-1"`, `"grant_id":"grant-2"`, 1),
		strings.Replace(validRevocation, `,"replayed":false`, ``, 1),
	} {
		var result consentmodel.ProjectExecutionConsentRevocationResult
		if err := decodeStrict([]byte(malformed), &result); err == nil &&
			validProjectExecutionConsentRevocation([]byte(malformed), result, "grant-1") {
			t.Fatalf("malformed revocation receipt accepted: %s", malformed)
		}
	}
	tooLateRevocation := strings.Replace(validRevocation, `"revoked_at_ms":300`, `"revoked_at_ms":9223372036854775808`, 1)
	var outOfRangeRevocation consentmodel.ProjectExecutionConsentRevocationResult
	if err := decodeStrict([]byte(tooLateRevocation), &outOfRangeRevocation); err == nil &&
		validProjectExecutionConsentRevocation([]byte(tooLateRevocation), outOfRangeRevocation, "grant-1") {
		t.Fatal("out-of-range revocation timestamp accepted")
	}
}

func TestProjectExecutionConsentRuntimeValidationErrorIsPreserved(t *testing.T) {
	appState := filepath.Join(t.TempDir(), "app-state")
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, appState)
	makeDirectory(t, runtimeState)
	executable := filepath.Join(t.TempDir(), "runtime-rpc")
	writeFake(t, executable, errorResponseScript(filepath.Join(runtimeState, "hub.sqlite3"), "invalid_project_execution_consent_request"))
	client := newConsentTestClient(t, executable, appState, runtimeState)
	_, err := client.RevokeProjectExecutionConsent(context.Background(), consentTestOwner(), "grant-1", "key")
	if err == nil || err.(*Error).Code != "invalid_project_execution_consent_request" {
		t.Fatalf("consent validation RPC error=%v", err)
	}
}

func newConsentTestClient(t *testing.T, executable, appState, runtimeState string) *Client {
	t.Helper()
	client, err := New(Config{Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState})
	if err != nil {
		t.Fatal(err)
	}
	return client
}

func consentTestOwner() model.Owner {
	return model.Owner{Issuer: "https://identity.example", Subject: "account-42", TenantID: "tenant-slate"}
}
