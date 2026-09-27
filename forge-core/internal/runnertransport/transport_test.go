package runnertransport

import (
	"encoding/json"
	"strings"
	"testing"
)

const (
	// secret-scan:ignore — deterministic HMAC fixture, never used outside this test.
	testSecret  = "runner-secret-for-transport-tests"
	testMethod  = "POST"
	testPath    = "/heartbeat"
	testTS      = int64(1_700_000_000)
	testNonce   = "nonce-0001"
	testPayload = `{"device_id":"runner-1","dynamic_state":{"cpu":2},"running_attempts":[]}`
)

func TestSignAndVerifyMatchesD3Binding(t *testing.T) {
	signature, err := Sign(testSecret, testMethod, testPath, testTS, testNonce, []byte(testPayload))
	if err != nil {
		t.Fatalf("sign: %v", err)
	}
	const wantPythonSignature = "7f773e565848a1fe8a4bd162d2832720e66fa1aa19d2bb1f529a430c1b97b772"
	if signature != wantPythonSignature {
		t.Fatalf("signature=%q, want Python D3 signature %q", signature, wantPythonSignature)
	}
	if len(signature) != MaxSignatureBytes || !isLowerHex(signature) {
		t.Fatalf("signature=%q", signature)
	}
	cache := NewReplayCache(4)
	observation, err := Verify(testSecret, testMethod, testPath, Envelope{
		TS: testTS, Nonce: testNonce, Sig: signature, Payload: json.RawMessage(testPayload),
	}, testTS, cache)
	if err != nil {
		t.Fatalf("verify: %v", err)
	}
	if observation.SchemaVersion != SchemaVersion || observation.EvaluationMode != EvaluationMode ||
		observation.Method != testMethod || observation.Path != testPath || observation.Timestamp != testTS ||
		observation.Nonce != testNonce || !observation.ReplayChecked || !observation.PreviewOnly ||
		observation.Authority != (Authority{}) || observation.PayloadBytes != len(testPayload) {
		t.Fatalf("unexpected observation: %#v", observation)
	}
	if observation.PayloadSHA256 == "" {
		t.Fatal("payload digest is empty")
	}
}

func TestSignPreservesPythonUnicodeAndHTMLCanonicalization(t *testing.T) {
	payload := []byte(`{"text":"<你好>&"}`)
	signature, err := Sign(testSecret, testMethod, testPath, testTS, "nonce-unicode", payload)
	if err != nil {
		t.Fatal(err)
	}
	const wantPythonSignature = "8e97fd7bd57d39eacb7332cf973ec8867ad8ec016a4583a97eed31fe79cf977e"
	if signature != wantPythonSignature {
		t.Fatalf("signature=%q, want Python D3 signature %q", signature, wantPythonSignature)
	}
}

func TestVerifyRejectsReplayAndDoesNotPoisonCacheWithBadSignature(t *testing.T) {
	signature, err := Sign(testSecret, testMethod, testPath, testTS, testNonce, []byte(testPayload))
	if err != nil {
		t.Fatal(err)
	}
	cache := NewReplayCache(2)
	bad := Envelope{TS: testTS, Nonce: testNonce, Sig: strings.Repeat("0", MaxSignatureBytes), Payload: json.RawMessage(testPayload)}
	if _, err := Verify(testSecret, testMethod, testPath, bad, testTS, cache); err != ErrInvalidSignature {
		t.Fatalf("bad signature error=%v", err)
	}
	good := bad
	good.Sig = signature
	if _, err := Verify(testSecret, testMethod, testPath, good, testTS, cache); err != nil {
		t.Fatalf("valid request after bad signature: %v", err)
	}
	if _, err := Verify(testSecret, testMethod, testPath, good, testTS, cache); err != ErrReplay {
		t.Fatalf("replay error=%v", err)
	}
}

func TestVerifyRejectsTimestampAndBindingDrift(t *testing.T) {
	signature, err := Sign(testSecret, testMethod, testPath, testTS, testNonce, []byte(testPayload))
	if err != nil {
		t.Fatal(err)
	}
	base := Envelope{TS: testTS, Nonce: testNonce, Sig: signature, Payload: json.RawMessage(testPayload)}
	tests := []struct {
		name   string
		method string
		path   string
		now    int64
		want   Error
	}{
		{name: "stale", method: testMethod, path: testPath, now: testTS + TimestampWindowSeconds + 1, want: ErrStaleTimestamp},
		{name: "method", method: "GET", path: testPath, now: testTS, want: ErrInvalidSignature},
		{name: "path", method: testMethod, path: "/other", now: testTS, want: ErrInvalidSignature},
	}
	for _, testCase := range tests {
		t.Run(testCase.name, func(t *testing.T) {
			request := base
			if testCase.name == "method" || testCase.name == "path" {
				// The signature is bound to the original method/path. A changed
				// route must fail at the HMAC comparison, not reach a handler.
				request.Sig = base.Sig
			}
			_, err := Verify(testSecret, testCase.method, testCase.path, request, testCase.now, nil)
			if err != testCase.want {
				t.Fatalf("error=%v, want %v", err, testCase.want)
			}
		})
	}
}

func TestDecodeEnvelopeRejectsShapeAndCanonicalDrift(t *testing.T) {
	signature, err := Sign(testSecret, testMethod, testPath, testTS, testNonce, []byte(testPayload))
	if err != nil {
		t.Fatal(err)
	}
	valid := `{"ts":1700000000,"nonce":"nonce-0001","sig":"` + signature + `","payload":{"device_id":"runner-1","dynamic_state":{"cpu":2},"running_attempts":[]}}`
	if _, err := DecodeEnvelope([]byte(valid)); err != nil {
		t.Fatalf("valid envelope: %v", err)
	}
	for name, value := range map[string]string{
		"duplicate":          `{"ts":1700000000,"nonce":"nonce-0001","sig":"` + signature + `","sig":"` + signature + `","payload":{}}`,
		"unknown":            `{"ts":1700000000,"nonce":"nonce-0001","sig":"` + signature + `","payload":{},"extra":true}`,
		"trailing":           valid + ` {}`,
		"whitespace payload": `{"ts":1700000000,"nonce":"nonce-0001","sig":"` + signature + `","payload":{"device_id": "runner-1","dynamic_state":{"cpu":2},"running_attempts":[]}}`,
	} {
		t.Run(name, func(t *testing.T) {
			if _, err := DecodeEnvelope([]byte(value)); err == nil {
				t.Fatal("malformed envelope was accepted")
			}
		})
	}
}

func TestReplayCacheIsBounded(t *testing.T) {
	cache := NewReplayCache(2)
	for _, nonce := range []string{"one", "two", "three"} {
		if err := cache.CheckAndRemember(nonce, testTS); err != nil {
			t.Fatalf("remember %q: %v", nonce, err)
		}
	}
	if err := cache.CheckAndRemember("three", testTS); err != ErrReplay {
		t.Fatalf("latest nonce error=%v", err)
	}
	// The bounded cache is allowed to cold-start after reaching its cap.
	if err := cache.CheckAndRemember("one", testTS); err != nil {
		t.Fatalf("evicted nonce error=%v", err)
	}
}

func TestSortedAuthorityKeysIsStable(t *testing.T) {
	keys := SortedAuthorityKeys()
	if len(keys) != 7 || keys[0] != "audit_published" || keys[len(keys)-1] != "reservation_created" {
		t.Fatalf("keys=%v", keys)
	}
}
