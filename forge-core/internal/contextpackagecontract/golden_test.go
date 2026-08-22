package contextpackagecontract

import (
	"bytes"
	"testing"
)

func TestGoldenFixture(t *testing.T) {
	fixture := loadFixture(t)
	requestJSON, err := CanonicalRequestJSON(&fixture.Request)
	if err != nil {
		t.Fatalf("CanonicalRequestJSON: %v", err)
	}
	decoded, err := DecodeCanonicalRequest(requestJSON)
	if err != nil {
		t.Fatalf("DecodeCanonicalRequest: %v", err)
	}
	assembled, err := Assemble(decoded, byteCounter{})
	if err != nil {
		t.Fatalf("Assemble: %v", err)
	}
	actual, err := CanonicalPackageJSON(assembled)
	if err != nil {
		t.Fatalf("CanonicalPackageJSON(actual): %v", err)
	}
	expected, err := CanonicalPackageJSON(&fixture.ExpectedPackage)
	if err != nil {
		t.Fatalf("CanonicalPackageJSON(expected): %v", err)
	}
	if !bytes.Equal(actual, expected) {
		t.Fatalf("package differs from cross-language golden\nactual:   %s\nexpected: %s", actual, expected)
	}
	if err := ValidatePackage(decoded, assembled, byteCounter{}); err != nil {
		t.Fatalf("ValidatePackage: %v", err)
	}
}

func TestGoldenIdentities(t *testing.T) {
	request := validRequest(t)
	requestDigest, err := RequestSHA256(request)
	if err != nil {
		t.Fatal(err)
	}
	cacheDigest, err := CacheKeySHA256(request)
	if err != nil {
		t.Fatal(err)
	}
	if requestDigest != "a5918fa6b07d90eb9e799eb422506260f7e925f0b6b81be2c786e0f54e06587b" {
		t.Fatalf("request digest = %s", requestDigest)
	}
	if cacheDigest != "20773cb1237ca42ef07a0b36570647283640f3d96dfcec9cc91fd53fa1c10fc4" {
		t.Fatalf("cache digest = %s", cacheDigest)
	}
}
