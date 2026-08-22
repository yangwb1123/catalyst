package contextpackagecontract

import (
	"bytes"
	"testing"
)

func TestDecodeCanonicalPackageStrictRoundTrip(t *testing.T) {
	fixture := loadFixture(t)
	canonical, err := CanonicalPackageJSON(&fixture.ExpectedPackage)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := DecodeCanonicalPackage(canonical)
	if err != nil {
		t.Fatal(err)
	}
	reencoded, err := CanonicalPackageJSON(decoded)
	if err != nil || !bytes.Equal(canonical, reencoded) {
		t.Fatalf("package round trip failed: %v", err)
	}
}

func TestDecodeCanonicalPackageRejectsWireDrift(t *testing.T) {
	fixture := loadFixture(t)
	canonical, err := CanonicalPackageJSON(&fixture.ExpectedPackage)
	if err != nil {
		t.Fatal(err)
	}
	unknown := bytes.Replace(canonical, []byte(`{"accounting":`), []byte(`{"added":1,"accounting":`), 1)
	for name, value := range map[string][]byte{
		"duplicate":    []byte(`{"api_version":1,"api_version":2}`),
		"noncanonical": append([]byte(" "), canonical...),
		"oversized":    bytes.Repeat([]byte(" "), maxPackageBytes+1),
		"unknown":      unknown,
	} {
		t.Run(name, func(t *testing.T) {
			if _, err := DecodeCanonicalPackage(value); err == nil {
				t.Fatal("expected strict package decode failure")
			}
		})
	}
}

func TestPackageShapeRejectsLaneAndResourceEscalation(t *testing.T) {
	fixture := loadFixture(t)
	packageValue := fixture.ExpectedPackage
	snippet := packageValue.Lanes.UntrustedData[0]
	snippet.Lane = "trusted_context"
	packageValue.Lanes.UntrustedData = nil
	packageValue.Lanes.TrustedContext = append(packageValue.Lanes.TrustedContext, snippet)
	if _, err := DecodeCanonicalPackage(mustPackageNodeJSON(t, &packageValue)); err == nil {
		t.Fatal("expected lane escalation rejection")
	}

	packageValue = fixture.ExpectedPackage
	packageValue.Accounting.CandidateCount = 65
	if _, err := CanonicalPackageJSON(&packageValue); err == nil {
		t.Fatal("expected candidate source bound rejection")
	}
	packageValue = fixture.ExpectedPackage
	packageValue.RedactionReceipts[0].Ranges[0].EndByte = 131073
	if _, err := CanonicalPackageJSON(&packageValue); err == nil {
		t.Fatal("expected redaction offset bound rejection")
	}
}

func TestDecodeCanonicalPackageRejectsEmptyCandidateSet(t *testing.T) {
	packageValue := loadFixture(t).ExpectedPackage
	packageValue.Accounting = Accounting{}
	packageValue.Freshness.ExpiresAtUnixMS = nil
	packageValue.Lanes = emptyLanes()
	packageValue.Omissions = []Omission{}
	packageValue.RedactionReceipts = []RedactionReceipt{}
	resealStandalonePackage(t, &packageValue)
	if _, err := DecodeCanonicalPackage(mustPackageNodeJSON(t, &packageValue)); err == nil {
		t.Fatal("expected empty candidate set rejection")
	}
}

func resealStandalonePackage(t *testing.T, packageValue *ContextPackage) {
	t.Helper()
	projection, err := canonicalJSON(projectionNode(packageValue.Lanes))
	if err != nil {
		t.Fatal(err)
	}
	packageValue.ProjectionSHA256 = domainDigest(projectionDigestDomain, projection)
	packageValue.ContextSHA256 = ""
	payload, err := canonicalContextPayloadJSON(packageValue)
	if err != nil {
		t.Fatal(err)
	}
	packageValue.ContextSHA256 = domainDigest(contextDigestDomain, payload)
}

func mustPackageNodeJSON(t *testing.T, packageValue *ContextPackage) []byte {
	t.Helper()
	value, err := canonicalJSON(packageNode(packageValue, packageValue.ContextSHA256))
	if err != nil {
		t.Fatal(err)
	}
	return value
}
