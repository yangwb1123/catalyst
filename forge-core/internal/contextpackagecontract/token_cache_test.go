package contextpackagecontract

import (
	"bytes"
	"testing"
)

type wrongCounter struct{}

func (wrongCounter) Identity() TokenizerIdentity {
	return TokenizerIdentity{TokenizerID: "wrong", TokenizerSHA256: utf8CounterHash}
}

func (wrongCounter) Count(value []byte) (uint64, error) { return uint64(len(value)), nil }

type intermediateFailCounter struct{}

func (intermediateFailCounter) Identity() TokenizerIdentity {
	return TokenizerIdentity{TokenizerID: utf8CounterID, TokenizerSHA256: utf8CounterHash}
}

type trackingCounter struct{ calls int }

func (*trackingCounter) Identity() TokenizerIdentity {
	return TokenizerIdentity{TokenizerID: utf8CounterID, TokenizerSHA256: utf8CounterHash}
}

type changingIdentityCounter struct{ identities int }

func (counter *changingIdentityCounter) Identity() TokenizerIdentity {
	counter.identities++
	identity := TokenizerIdentity{TokenizerID: utf8CounterID, TokenizerSHA256: utf8CounterHash}
	if counter.identities > 1 {
		identity.TokenizerID = "changed"
	}
	return identity
}

func (*changingIdentityCounter) Count(value []byte) (uint64, error) {
	return uint64(len(value)), nil
}

func (counter *trackingCounter) Count(value []byte) (uint64, error) {
	counter.calls++
	return uint64(len(value)), nil
}

func (intermediateFailCounter) Count(value []byte) (uint64, error) {
	if bytes.Contains(value, []byte("source-01-policy")) &&
		!bytes.Contains(value, []byte("source-02-decision")) {
		return 0, errCounter
	}
	return uint64(len(value)), nil
}

func TestCounterIdentityAndFailuresFailClosed(t *testing.T) {
	request := validRequest(t)
	if _, err := Assemble(request, nil); err == nil {
		t.Fatal("expected nil counter failure")
	}
	if _, err := Assemble(request, wrongCounter{}); err == nil {
		t.Fatal("expected identity mismatch")
	}
	if _, err := Assemble(request, byteCounter{err: errCounter}); err == nil {
		t.Fatal("expected counter failure")
	}
	if _, err := Assemble(request, &changingIdentityCounter{}); err == nil {
		t.Fatal("expected changing counter identity failure")
	}
}

func TestRequiredSourcesAreCountedIncrementally(t *testing.T) {
	request := validRequest(t)
	request.Sources[1].Required = true
	if _, err := Assemble(request, intermediateFailCounter{}); err == nil {
		t.Fatal("expected intermediate required projection counter failure")
	}
}

func TestCacheAndRequestDigestsBindExactRequest(t *testing.T) {
	request := validRequest(t)
	requestDigest, err := RequestSHA256(request)
	if err != nil {
		t.Fatal(err)
	}
	cacheDigest, err := CacheKeySHA256(request)
	if err != nil {
		t.Fatal(err)
	}
	request.Sources[1].Priority++
	changedRequest, err := RequestSHA256(request)
	if err != nil {
		t.Fatal(err)
	}
	changedCache, err := CacheKeySHA256(request)
	if err != nil {
		t.Fatal(err)
	}
	if requestDigest == changedRequest || cacheDigest == changedCache || requestDigest == cacheDigest {
		t.Fatal("domain-separated exact request identities did not change independently")
	}
}

func TestValidatePackageDemandsExactReassembly(t *testing.T) {
	request := validRequest(t)
	packageValue, err := Assemble(request, byteCounter{})
	if err != nil {
		t.Fatal(err)
	}
	packageValue.Accounting.ActualTokens++
	if err := ValidatePackage(request, packageValue, byteCounter{}); err == nil {
		t.Fatal("expected mutated accounting rejection")
	}
}

func TestValidateCacheHitChecksKeyBeforeCounter(t *testing.T) {
	request := validRequest(t)
	packageValue, err := Assemble(request, byteCounter{})
	if err != nil {
		t.Fatal(err)
	}
	packageValue.CacheKeySHA256 = "0" + packageValue.CacheKeySHA256[1:]
	counter := &trackingCounter{}
	if err := ValidateCacheHit(request, packageValue, counter); err == nil {
		t.Fatal("expected cache key mismatch")
	}
	if counter.calls != 0 {
		t.Fatalf("counter called %d times before cache key rejection", counter.calls)
	}
}
