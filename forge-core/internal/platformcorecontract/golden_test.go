package platformcorecontract

import (
	"bytes"
	"reflect"
	"testing"
)

func TestGoldenCanonicalDigestsAndRoundTrips(t *testing.T) {
	fixture := loadGoldenFixture(t)
	if fixture.APIVersion != "forge.platform-core-envelope-fixture/v1" {
		t.Fatalf("fixture api_version = %q", fixture.APIVersion)
	}
	assertArtifactGolden(t, fixture)
	assertCommandGolden(t, fixture)
	assertEventGolden(t, fixture)
}

func assertArtifactGolden(t *testing.T, fixture goldenFixture) {
	t.Helper()
	canonical, err := CanonicalArtifactRefJSON(&fixture.ArtifactRef)
	if err != nil {
		t.Fatal(err)
	}
	digest, err := ArtifactRefSHA256(&fixture.ArtifactRef)
	if err != nil || digest != fixture.Expected.ArtifactRefSHA256 {
		t.Fatalf("ArtifactRef digest = %q, err %v", digest, err)
	}
	decoded, err := DecodeCanonicalArtifactRef(canonical)
	if err != nil || !reflect.DeepEqual(decoded, &fixture.ArtifactRef) {
		t.Fatalf("ArtifactRef round trip failed: %v", err)
	}
}

func assertCommandGolden(t *testing.T, fixture goldenFixture) {
	t.Helper()
	canonical, err := CanonicalCommandEnvelopeJSON(&fixture.CommandEnvelope)
	if err != nil {
		t.Fatal(err)
	}
	digest, err := CommandEnvelopeSHA256(&fixture.CommandEnvelope)
	if err != nil || digest != fixture.Expected.CommandEnvelopeSHA256 {
		t.Fatalf("CommandEnvelope digest = %q, err %v", digest, err)
	}
	decoded, err := DecodeCanonicalCommandEnvelope(canonical)
	if err != nil || !reflect.DeepEqual(decoded, &fixture.CommandEnvelope) {
		t.Fatalf("CommandEnvelope round trip failed: %v", err)
	}
	if !bytes.HasPrefix(canonical, []byte(`{"actor_ref":`)) {
		t.Fatalf("CommandEnvelope is not key-sorted: %s", canonical)
	}
}

func assertEventGolden(t *testing.T, fixture goldenFixture) {
	t.Helper()
	canonical, err := CanonicalEventEnvelopeJSON(&fixture.EventEnvelope)
	if err != nil {
		t.Fatal(err)
	}
	digest, err := EventEnvelopeSHA256(&fixture.EventEnvelope)
	if err != nil || digest != fixture.Expected.EventEnvelopeSHA256 {
		t.Fatalf("EventEnvelope digest = %q, err %v", digest, err)
	}
	decoded, err := DecodeCanonicalEventEnvelope(canonical)
	if err != nil || !reflect.DeepEqual(decoded, &fixture.EventEnvelope) {
		t.Fatalf("EventEnvelope round trip failed: %v", err)
	}
}
