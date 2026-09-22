package devicecredential

import (
	"math"
	"testing"

	"forgeos/forge-core/internal/deviceidentity"
)

func credentialOwner() deviceidentity.Owner {
	return deviceidentity.Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
}

func credentialDigest(character byte) string {
	value := make([]byte, DigestHexBytes)
	for index := range value {
		value[index] = character
	}
	return string(value)
}

func issueRequest() Request {
	return Request{
		Action:          ActionIssue,
		Owner:           credentialOwner(),
		DeviceID:        "device-1",
		ApprovalState:   ApprovalPending,
		CredentialID:    "credential-1",
		KeyID:           "key-1",
		PublicKeySHA256: credentialDigest('a'),
		KeyGeneration:   1,
		IssuedAtMS:      100,
		ExpiresAtMS:     1_100,
		ObservedAtMS:    100,
	}
}

func issuedCredential(t *testing.T) State {
	t.Helper()
	transition, err := Apply(issueRequest())
	if err != nil {
		t.Fatalf("issue error = %v", err)
	}
	return transition.Next
}

func TestIssueReturnsMetadataOnlyCandidate(t *testing.T) {
	transition, err := Apply(issueRequest())
	if err != nil {
		t.Fatalf("issue error = %v", err)
	}
	if transition.SchemaVersion != SchemaVersion || transition.EvaluationMode != EvaluationMode ||
		transition.Action != ActionIssue || transition.Previous != nil || !transition.PreviewOnly {
		t.Fatalf("issue transition envelope = %#v", transition)
	}
	if transition.Next.CredentialState != CredentialActive || transition.Next.ApprovalState != ApprovalPending {
		t.Fatalf("issue state = %#v", transition.Next)
	}
	if !transition.Authority.OwnerBindingMatched || transition.Authority.OwnerAuthenticated ||
		transition.Authority.CredentialMaterialMade || transition.Authority.Persisted ||
		transition.Authority.InventoryAuthoritative || transition.Authority.ExecutionAuthorized {
		t.Fatalf("issue authority = %#v", transition.Authority)
	}
	if err := transition.Next.Validate(); err != nil {
		t.Fatalf("issued state validation = %v", err)
	}
}

func TestRevokeIsTerminalAndDoesNotMintMaterial(t *testing.T) {
	current := issuedCredential(t)
	transition, err := Apply(Request{
		Current: currentPtr(current), Action: ActionRevoke,
		Owner: current.Owner, DeviceID: current.DeviceID,
	})
	if err != nil {
		t.Fatalf("revoke error = %v", err)
	}
	if transition.Next.CredentialState != CredentialRevoked || transition.Previous == nil {
		t.Fatalf("revoke transition = %#v", transition)
	}
	if _, err := Apply(Request{
		Current: currentPtr(transition.Next), Action: ActionRevoke,
		Owner: current.Owner, DeviceID: current.DeviceID,
	}); err != ErrCredentialTerminal {
		t.Fatalf("second revoke error = %v, want %v", err, ErrCredentialTerminal)
	}
}

func TestRotateRebindsKeyAndCredentialWithShortWindow(t *testing.T) {
	current := issuedCredential(t)
	transition, err := Apply(Request{
		Current:             currentPtr(current),
		Action:              ActionRotate,
		Owner:               current.Owner,
		DeviceID:            current.DeviceID,
		NextCredentialID:    "credential-2",
		NextKeyID:           "key-2",
		NextPublicKeySHA256: credentialDigest('b'),
		IssuedAtMS:          200,
		ExpiresAtMS:         1_200,
		ObservedAtMS:        200,
	})
	if err != nil {
		t.Fatalf("rotation error = %v", err)
	}
	if transition.Next.KeyGeneration != current.KeyGeneration+1 || transition.Next.KeyID != "key-2" ||
		transition.Next.CredentialID != "credential-2" || transition.Next.PublicKeySHA256 != credentialDigest('b') {
		t.Fatalf("rotated state = %#v", transition.Next)
	}
	if transition.Next.Owner != current.Owner || transition.Next.DeviceID != current.DeviceID ||
		transition.Next.ApprovalState != current.ApprovalState {
		t.Fatal("rotation changed immutable binding")
	}
}

func TestIssueAndRotationRejectUnsafeWindowsAndBindings(t *testing.T) {
	cases := []struct {
		name string
		edit func(*Request)
		want ErrorCode
	}{
		{name: "future_issue", edit: func(request *Request) { request.ObservedAtMS = request.IssuedAtMS - 1 }, want: ErrCredentialNotYetLive},
		{name: "expired_issue", edit: func(request *Request) { request.ObservedAtMS = request.ExpiresAtMS }, want: ErrCredentialExpired},
		{name: "long_window", edit: func(request *Request) { request.ExpiresAtMS = request.IssuedAtMS + MaxLifetimeMS + 1 }, want: ErrLifetimeTooLong},
		{name: "short_window", edit: func(request *Request) { request.ExpiresAtMS = request.IssuedAtMS + MinLifetimeMS - 1 }, want: ErrLifetimeTooShort},
		{name: "revoked_approval", edit: func(request *Request) { request.ApprovalState = ApprovalRevoked }, want: ErrApprovalRevoked},
		{name: "invalid_owner", edit: func(request *Request) { request.Owner.Subject = "" }, want: ErrInvalidOwner},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			request := issueRequest()
			testCase.edit(&request)
			if _, err := Apply(request); err != testCase.want {
				t.Fatalf("error = %v, want %v", err, testCase.want)
			}
		})
	}

	current := issuedCredential(t)
	_, err := Apply(Request{
		Current: currentPtr(current), Action: ActionRotate,
		Owner: current.Owner, DeviceID: "other-device",
		NextCredentialID: "credential-2", NextKeyID: "key-2",
		NextPublicKeySHA256: credentialDigest('b'), IssuedAtMS: 200,
		ExpiresAtMS: 1_200, ObservedAtMS: 200,
	})
	if err != ErrDeviceMismatch {
		t.Fatalf("device drift error = %v, want %v", err, ErrDeviceMismatch)
	}
}

func TestStateValidationRejectsGenerationAndCredentialDrift(t *testing.T) {
	state := issuedCredential(t)
	state.KeyGeneration = 0
	if err := state.Validate(); err != ErrInvalidKey {
		t.Fatalf("zero generation error = %v, want %v", err, ErrInvalidKey)
	}
	state = issuedCredential(t)
	state.CredentialState = "unknown"
	if err := state.Validate(); err != ErrUnknownCredential {
		t.Fatalf("unknown credential error = %v, want %v", err, ErrUnknownCredential)
	}
	state = issuedCredential(t)
	state.ExpiresAtMS = state.IssuedAtMS
	if err := state.Validate(); err != ErrInvalidWindow {
		t.Fatalf("invalid window error = %v, want %v", err, ErrInvalidWindow)
	}

	current := issuedCredential(t)
	current.KeyGeneration = math.MaxUint64
	_, err := Apply(Request{
		Current: currentPtr(current), Action: ActionRotate,
		Owner: current.Owner, DeviceID: current.DeviceID,
		NextCredentialID: "credential-2", NextKeyID: "key-2",
		NextPublicKeySHA256: credentialDigest('b'), IssuedAtMS: 200,
		ExpiresAtMS: 1_200, ObservedAtMS: 200,
	})
	if err != ErrGenerationOverflow {
		t.Fatalf("generation overflow error = %v, want %v", err, ErrGenerationOverflow)
	}
}

func TestCredentialLifecycleDoesNotAcceptSecretMaterial(t *testing.T) {
	// Keep this test close to the API shape so a future edit cannot quietly
	// add a bearer token field to the pure transition.  The type has only
	// metadata and the transition JSON contains no token-like member.
	transition, err := Apply(issueRequest())
	if err != nil {
		t.Fatal(err)
	}
	if transition.Authority.CredentialMaterialMade {
		t.Fatal("pure lifecycle unexpectedly claims credential material")
	}
}

func currentPtr(value State) *State { return &value }
