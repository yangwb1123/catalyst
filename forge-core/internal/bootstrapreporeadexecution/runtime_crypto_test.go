package bootstrapreporeadexecution

import (
	"bytes"
	"crypto/ed25519"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"testing"
)

type testKeys struct {
	public []string
	seeds  [][]byte
}

func generateTestKeys(t *testing.T, count int) testKeys {
	t.Helper()
	result := testKeys{public: make([]string, count), seeds: make([][]byte, count)}
	for index := 0; index < count; index++ {
		publicKey, privateKey, err := ed25519.GenerateKey(rand.Reader)
		if err != nil {
			t.Fatal(err)
		}
		result.public[index] = base64.RawURLEncoding.EncodeToString(publicKey)
		result.seeds[index] = append([]byte(nil), privateKey.Seed()...)
	}
	return result
}

func sealTestDocument(t *testing.T, document map[string]any, digestDomain []byte,
	digestField string, signed bool, derivedID string, seed, signatureDomain []byte) {
	t.Helper()
	preimage := cloneTestNode(t, document).(map[string]any)
	preimage[digestField] = ""
	if derivedID != "" {
		preimage[derivedID] = ""
	}
	if signed {
		preimage["signature"].(map[string]any)["signature_base64url"] = ""
	}
	digest := digestTestNode(t, digestDomain, preimage)
	document[digestField] = digest
	if derivedID != "" {
		document[derivedID] = "bootstrap-repo-read-invocation-" + digest
	}
	if signed {
		document["signature"].(map[string]any)["signature_base64url"] =
			signTestDigest(t, seed, signatureDomain, digest)
	}
}

func signTestDigest(t *testing.T, seed, domain []byte, digest string) string {
	t.Helper()
	raw, err := hex.DecodeString(digest)
	if err != nil || len(raw) != sha256.Size {
		t.Fatalf("invalid digest %q", digest)
	}
	privateKey := ed25519.NewKeyFromSeed(seed)
	message := append(append([]byte(nil), domain...), raw...)
	signature := ed25519.Sign(privateKey, message)
	for index := range privateKey {
		privateKey[index] = 0
	}
	return base64.RawURLEncoding.EncodeToString(signature)
}

func digestTestNode(t *testing.T, domain []byte, value any) string {
	t.Helper()
	encoded := canonicalTestNode(t, value)
	digest := sha256.New()
	_, _ = digest.Write(domain)
	_, _ = digest.Write(encoded)
	return hex.EncodeToString(digest.Sum(nil))
}

func canonicalTestNode(t *testing.T, value any) []byte {
	t.Helper()
	encoded, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return encoded
}

func cloneTestNode(t *testing.T, value any) any {
	t.Helper()
	decoder := json.NewDecoder(bytes.NewReader(canonicalTestNode(t, value)))
	decoder.UseNumber()
	var result any
	if err := decoder.Decode(&result); err != nil {
		t.Fatal(err)
	}
	return result
}
