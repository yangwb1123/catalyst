//go:build unix

package authenticatedadrapprovalauthority

import "testing"

func TestHalfOpenTimeAndExternalTrustBoundaries(t *testing.T) {
	t.Run("request start included", func(t *testing.T) {
		fixture := newServiceFixture(t)
		trust := fixture.trust()
		trust.ObservedAtUnixMS = fixture.request["requested_at_unix_ms"].(int64)
		if _, err := AuthorizeAndStore(fixture.config(t), fixture.encoded(t), trust); err != nil {
			t.Fatalf("inclusive request start failed: %v", err)
		}
	})
	t.Run("before request start", func(t *testing.T) {
		fixture := newServiceFixture(t)
		trust := fixture.trust()
		trust.ObservedAtUnixMS = fixture.request["requested_at_unix_ms"].(int64) - 1
		config := fixture.config(t)
		_, err := AuthorizeAndStore(config, fixture.encoded(t), trust)
		assertCode(t, err, codeTimeRejected)
		assertStateLeavesAbsent(t, config)
	})
	t.Run("trust epoch", func(t *testing.T) {
		fixture := newServiceFixture(t)
		trust := fixture.trust()
		trust.PinnedTrustEpoch++
		config := fixture.config(t)
		_, err := AuthorizeAndStore(config, fixture.encoded(t), trust)
		assertCode(t, err, codeTrustRootRejected)
		assertStateLeavesAbsent(t, config)
	})
	t.Run("revocation digest high-water", func(t *testing.T) {
		fixture := newServiceFixture(t)
		trust := fixture.trust()
		trust.RevocationHighWaterSHA256 =
			"0000000000000000000000000000000000000000000000000000000000000000"
		config := fixture.config(t)
		_, err := AuthorizeAndStore(config, fixture.encoded(t), trust)
		assertCode(t, err, codeRevocationRejected)
		assertStateLeavesAbsent(t, config)
	})
	t.Run("revocation sequence high-water", func(t *testing.T) {
		fixture := newServiceFixture(t)
		trust := fixture.trust()
		trust.RevocationHighWaterSequence++
		config := fixture.config(t)
		_, err := AuthorizeAndStore(config, fixture.encoded(t), trust)
		assertCode(t, err, codeRevocationRejected)
		assertStateLeavesAbsent(t, config)
	})
}

func TestStateSigningKeyRevocationFailsBeforeLock(t *testing.T) {
	fixture := newServiceFixture(t)
	stateKey := fixture.byUsage["approval_authorization_state_sign"]
	fixture.revocation["revoked_key_ids"] = []any{stateKey}
	sealFixtureRevocation(t, fixture, fixture.revocation)
	fixture.request["revocation_sha256"] = fixture.revocation["revocation_sha256"]
	fixture.sealRequest(t)
	config := fixture.config(t)
	result, err := AuthorizeAndStore(config, fixture.encoded(t), fixture.trust())
	if result != nil {
		t.Fatal("revoked state key returned output")
	}
	assertCode(t, err, codeAuthorizationNotCurrent)
	assertStateLeavesAbsent(t, config)
}
