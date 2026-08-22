package planningownership

import (
	"bytes"
	"crypto/sha256"
	"fmt"
	"os"
	"testing"
)

const (
	goldenFixturePath  = "../../../docs/contracts/fixtures/planning-capability-ownership-projection-v1.json"
	catalogFixturePath = "../../../docs/design/ai-engineering-os/capability-catalog.v1.yml"
	mappingFixturePath = "../../../docs/design/ai-engineering-os/capability-skill-map.v1.yml"
	goldenBytes        = 172733
	goldenSHA256       = "3d0a877bef0939cff5752fc5d602e0d3a90e19639308801008f9d2d9ff139f36"
	requestSHA256      = "3639c4d3ad21db93db254b7da2643d492ca39c4dda5438de426379cd70718cfa"
	projectionSHA256   = "53754ded32379d6520f3bd2b9d2956238731ad40c11124be457b724b4c150fa2"
)

type goldenFixture struct {
	requestRaw    []byte
	projectionRaw []byte
}

func TestCrossLanguageGoldenExactBytes(t *testing.T) {
	fixture := loadGolden(t)
	catalog := readFixture(t, catalogFixturePath)
	mapping := readFixture(t, mappingFixturePath)
	request, err := BuildRequest(catalog, mapping)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(request.CanonicalBytes(), fixture.requestRaw) {
		t.Fatal("Go request bytes differ from Python golden")
	}
	projection, err := Project(request)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(projection.CanonicalBytes(), fixture.projectionRaw) {
		t.Fatal("Go projection bytes differ from Python golden")
	}
	assertGoldenSemantics(t, request, projection)
}

func TestGoldenDecodersAndDefensiveCopies(t *testing.T) {
	fixture := loadGolden(t)
	request, err := DecodeRequest(fixture.requestRaw)
	if err != nil {
		t.Fatal(err)
	}
	projection, err := DecodeProjection(fixture.projectionRaw)
	if err != nil || ValidateProjection(projection) != nil {
		t.Fatalf("projection validation = %v", err)
	}
	mutateCopy(request.CanonicalBytes())
	mutateCopy(request.CatalogSourceBytes())
	mutateCopy(request.MappingSourceBytes())
	mutateCopy(projection.CanonicalBytes())
	if !bytes.Equal(request.CanonicalBytes(), fixture.requestRaw) ||
		!bytes.Equal(projection.CanonicalBytes(), fixture.projectionRaw) {
		t.Fatal("public byte accessors leaked mutable backing storage")
	}
}

func TestPublicProjectionAPIIsPureAndDefensivelyOwnsCallerBytes(t *testing.T) {
	catalog := readFixture(t, catalogFixturePath)
	mapping := readFixture(t, mappingFixturePath)
	catalogOriginal, mappingOriginal := cloneBytes(catalog), cloneBytes(mapping)
	request, err := BuildRequest(catalog, mapping)
	if err != nil {
		t.Fatal(err)
	}
	requestBytes := request.CanonicalBytes()
	catalog[0], mapping[0] = 'X', 'X'
	if !bytes.Equal(request.CatalogSourceBytes(), catalogOriginal) ||
		!bytes.Equal(request.MappingSourceBytes(), mappingOriginal) ||
		!bytes.Equal(request.CanonicalBytes(), requestBytes) {
		t.Fatal("BuildRequest retained caller-owned mutable source storage")
	}
	first, err := Project(request)
	if err != nil {
		t.Fatal(err)
	}
	second, err := Project(request)
	if err != nil || !bytes.Equal(first.CanonicalBytes(), second.CanonicalBytes()) ||
		!bytes.Equal(request.CanonicalBytes(), requestBytes) {
		t.Fatal("Project was nondeterministic or mutated its request")
	}
	firstCopy := first.CanonicalBytes()
	mutateCopy(firstCopy)
	if !bytes.Equal(first.CanonicalBytes(), second.CanonicalBytes()) {
		t.Fatal("Projection.CanonicalBytes leaked mutable storage")
	}
}

func assertGoldenSemantics(t *testing.T, request Request, projection Projection) {
	t.Helper()
	if request.document["request_sha256"] != requestSHA256 ||
		projection.document["projection_sha256"] != projectionSHA256 {
		t.Fatal("golden digest chain drifted")
	}
	coverage := projection.document["coverage"].(map[string]any)
	expected := map[string]int64{
		"catalog_node_count": 17, "capability_occurrence_count": 145,
		"unique_capability_count": 140, "mapping_package_count": 38,
		"mapped_capability_count": 140, "binding_count": 140,
	}
	for key, value := range expected {
		if coverage[key] != value {
			t.Fatalf("coverage %s = %v, want %d", key, coverage[key], value)
		}
	}
}

func loadGolden(t *testing.T) goldenFixture {
	t.Helper()
	raw := readFixture(t, goldenFixturePath)
	digest := fmt.Sprintf("%x", sha256.Sum256(raw))
	if len(raw) != goldenBytes || digest != goldenSHA256 || raw[len(raw)-1] != '\n' || raw[len(raw)-2] == '\n' {
		t.Fatalf("golden physical pin drifted: %d/%s", len(raw), digest)
	}
	root, err := parseCanonicalObject(raw[:len(raw)-1], maxProjectionBytes)
	if err != nil || requireKeys(root, []string{"api_version", "canonicalization", "projection", "request"}) != nil {
		t.Fatalf("golden envelope rejected: %v", err)
	}
	if root["api_version"] != "forgeos.planning-capability-ownership-projection-golden/v1" ||
		root["canonicalization"] != canonicalization {
		t.Fatal("golden envelope constants drifted")
	}
	requestRaw, err := canonicalJSON(root["request"])
	if err != nil {
		t.Fatal(err)
	}
	projectionRaw, err := canonicalJSON(root["projection"])
	if err != nil {
		t.Fatal(err)
	}
	return goldenFixture{requestRaw: requestRaw, projectionRaw: projectionRaw}
}

func readFixture(t *testing.T, path string) []byte {
	t.Helper()
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func mutateCopy(raw []byte) {
	if len(raw) > 0 {
		raw[0] ^= 0xff
	}
}
