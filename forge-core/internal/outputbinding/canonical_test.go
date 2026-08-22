package outputbinding

import (
	"bytes"
	"encoding/json"
	"fmt"
	"strings"
	"testing"
)

func TestEveryCanonicalDocumentUsesLexicographicObjectKeys(t *testing.T) {
	manifest := testManifest(t, ManifestItem{Bytes: 1, Path: "out", SHA256: testDigest("out")})
	policy := testPolicy(t)
	preflight := testPreflight(t, policy, manifest, 1)
	receipt := testReceipt(t, 1, nil)
	documents := []func() ([]byte, error){
		func() ([]byte, error) { return CanonicalManifestJSON(manifest) },
		func() ([]byte, error) { return CanonicalRuntimePolicyJSON(policy) },
		func() ([]byte, error) { return CanonicalPreflightJSON(preflight) },
		func() ([]byte, error) { return CanonicalReceiptJSON(receipt) },
	}
	for index, document := range documents {
		encoded, err := document()
		if err != nil {
			t.Fatalf("document %d: %v", index, err)
		}
		decoder := json.NewDecoder(bytes.NewReader(encoded))
		if err := checkCanonicalJSONNode(decoder); err != nil {
			t.Fatalf("document %d: %v", index, err)
		}
	}
}

func checkCanonicalJSONNode(decoder *json.Decoder) error {
	token, err := decoder.Token()
	if err != nil {
		return err
	}
	delimiter, container := token.(json.Delim)
	if !container {
		return nil
	}
	if delimiter == '[' {
		for decoder.More() {
			if err := checkCanonicalJSONNode(decoder); err != nil {
				return err
			}
		}
		_, err = decoder.Token()
		return err
	}
	if delimiter != '{' {
		return fmt.Errorf("unexpected delimiter %q", delimiter)
	}
	return checkCanonicalJSONObject(decoder)
}

func checkCanonicalJSONObject(decoder *json.Decoder) error {
	var prior string
	for decoder.More() {
		token, err := decoder.Token()
		key, ok := token.(string)
		if err != nil || !ok || (prior != "" && prior >= key) {
			return fmt.Errorf("object keys are not lexicographic after %q: %q", prior, key)
		}
		prior = key
		if err := checkCanonicalJSONNode(decoder); err != nil {
			return err
		}
	}
	_, err := decoder.Token()
	return err
}

func TestStrictDecoderRejectsNumbersEscapesBoundsAndNulls(t *testing.T) {
	manifest := testManifest(t, ManifestItem{Bytes: 1, Path: "out", SHA256: testDigest("out")})
	encoded, err := CanonicalManifestJSON(manifest)
	if err != nil {
		t.Fatal(err)
	}
	raw := string(encoded)
	mutations := [][]byte{
		[]byte(strings.Replace(raw, `"bytes":1`, `"bytes":1.0`, 1)),
		[]byte(strings.Replace(raw, `"bytes":1`, `"bytes":-0`, 1)),
		[]byte(strings.Replace(raw, `"path":"out"`, `"path":"\u006fut"`, 1)),
		[]byte(strings.Replace(raw, `"items":[`, `"items":null,"discarded":[`, 1)),
	}
	for index, mutation := range mutations {
		if _, err := DecodeCanonicalManifest(mutation); err == nil {
			t.Fatalf("strict decoder mutation %d was accepted", index)
		}
	}
	if _, err := DecodeCanonicalPreflight(bytes.Repeat([]byte("x"), maxPreflightBytes+1)); err == nil {
		t.Fatal("preflight byte ceiling was not enforced before parsing")
	}
}

func TestTypedBuildersRejectInvalidUTF8AndAggregateOverflow(t *testing.T) {
	policy := testPolicy(t)
	policy.Model = string([]byte{0xff})
	if _, err := SealRuntimePolicy(policy); err == nil {
		t.Fatal("invalid UTF-8 policy value was accepted")
	}
	items := []ManifestItem{
		{Bytes: maxArtifactBytes, Path: "a", SHA256: testDigest("a")},
		{Bytes: 1, Path: "b", SHA256: testDigest("b")},
	}
	if _, err := SealManifest(items); err == nil {
		t.Fatal("aggregate artifact byte overflow was accepted")
	}
}
