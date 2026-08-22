package legacygovernanceimportcontract

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

type testSource struct {
	kind string
	ref  string
	raw  []byte
}

func fixture(t *testing.T, name string) []byte {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", name)
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func testRequest(t *testing.T, sources []testSource) []byte {
	t.Helper()
	items := make([]any, 0, len(sources))
	for _, source := range sources {
		items = append(items, map[string]any{
			"byte_count": int64(len(source.raw)), "content_base64url": encodeBase64URL(source.raw),
			"content_sha256": shaBytes(source.raw), "source_kind": source.kind,
			"source_ref": source.ref,
		})
	}
	request := map[string]any{
		"api_version": requestAPI,
		"binding": map[string]any{
			"project_id": "fixture-project", "source_revision": "fixture-revision",
			"source_tree_sha256": strings.Repeat("0", 64),
		},
		"canonicalization": canonicalization, "kind": requestKind,
		"request_sha256": "", "sources": items,
	}
	digest, err := selfDigest(requestDomain, request, "request_sha256", maxRequestBytes, "request")
	if err != nil {
		t.Fatal(err)
	}
	request["request_sha256"] = digest
	encoded, err := canonicalJSON(request, maxRequestBytes-1, "request")
	if err != nil {
		t.Fatal(err)
	}
	return append(encoded, '\n')
}

func confidenceMemory(lexeme string) []byte {
	return []byte(fmt.Sprintf(
		`{"kind":"gap","topic":"t","detail":"d","iteration":1,"confidence":%s,"created_at_unix":2}`+"\n",
		lexeme))
}

func resealTestCandidate(t *testing.T, candidate map[string]any) {
	t.Helper()
	locator := map[string]any{
		"ordinal": candidate["ordinal"], "request_sha256": candidate["request_sha256"],
		"source_kind": candidate["source_kind"], "source_ref": candidate["source_ref"],
	}
	id, err := digestValue(candidateIDDomain, locator, maxRequestBytes, "candidate locator")
	if err != nil {
		t.Fatal(err)
	}
	candidate["candidate_id"] = id
	digest, err := selfDigest(candidateDomain, candidate, "candidate_sha256", maxViewBytes, "candidate")
	if err != nil {
		t.Fatal(err)
	}
	candidate["candidate_sha256"] = digest
}

func resealTestView(t *testing.T, view map[string]any) []byte {
	t.Helper()
	sourceSHA, err := digestValue(sourceSetDomain, view["sources"], maxViewBytes, "source set")
	if err != nil {
		t.Fatal(err)
	}
	view["source_set_sha256"] = sourceSHA
	digest, err := selfDigest(viewDomain, view, "view_sha256", maxViewBytes, "view")
	if err != nil {
		t.Fatal(err)
	}
	view["view_sha256"] = digest
	encoded, err := canonicalJSON(view, maxViewBytes, "view")
	if err != nil {
		t.Fatal(err)
	}
	return encoded
}

func testMemoryCandidate(t *testing.T, requestSHA string, ordinal int) (map[string]any, []byte) {
	t.Helper()
	entry := map[string]any{
		"kind": "gap", "topic": fmt.Sprintf("topic-%04d", ordinal), "detail": "d",
		"iteration": int64(ordinal), "created_at_unix": int64(ordinal),
	}
	raw, err := canonicalJSON(entry, maxMemoryLineBytes, "memory test line")
	if err != nil {
		t.Fatal(err)
	}
	request := map[string]any{"request_sha256": requestSHA}
	source := map[string]any{"source_kind": memoryKind, "source_ref": ".forge/memory.jsonl"}
	candidate, err := candidateBase(request, source, int64(ordinal), raw)
	if err != nil {
		t.Fatal(err)
	}
	for field, value := range map[string]any{
		"confidence":      map[string]any{"presence": "omitted", "raw_number_lexeme": nil},
		"created_at_unix": int64(ordinal), "declared_kind": "gap", "declared_source": nil,
		"declared_supersedes": nil, "declared_topic": entry["topic"], "detail": "d",
		"iteration": int64(ordinal), "legacy_format": nil,
	} {
		candidate[field] = value
	}
	if err := sealCandidate(candidate); err != nil {
		t.Fatal(err)
	}
	return candidate, raw
}

func memoryOnlyView(t *testing.T, count int) []byte {
	t.Helper()
	golden := fixture(t, "legacy-governance-read-import-view-v1.json")
	value, err := parseStrictJSON(golden[:len(golden)-1], maxViewBytes, false)
	if err != nil {
		t.Fatal(err)
	}
	view := value.(map[string]any)
	candidates := make([]any, 0, count)
	sourceRaw := bytes.NewBuffer(nil)
	for ordinal := 1; ordinal <= count; ordinal++ {
		candidate, raw := testMemoryCandidate(t, view["request_sha256"].(string), ordinal)
		candidates = append(candidates, candidate)
		sourceRaw.Write(raw)
		sourceRaw.WriteByte('\n')
	}
	view["candidates"] = candidates
	view["conflict_sets"], view["declared_supersessions"] = []any{}, []any{}
	view["sources"] = []any{map[string]any{
		"byte_count": int64(sourceRaw.Len()), "content_sha256": shaBytes(sourceRaw.Bytes()),
		"source_kind": memoryKind, "source_ref": ".forge/memory.jsonl",
	}}
	return resealTestView(t, view)
}

func TestProjectMatchesCrossLanguageGoldenExactly(t *testing.T) {
	request := fixture(t, "legacy-governance-read-import-request-v1.json")
	want := fixture(t, "legacy-governance-read-import-view-v1.json")
	got, err := Project(request)
	if err != nil {
		t.Fatal(err)
	}
	got = append(got, '\n')
	if !bytes.Equal(got, want) {
		t.Fatalf("Go projection differs from exact golden\ngot:  %s\nwant: %s", got, want)
	}
	if _, err := DecodeView(want[:len(want)-1]); err != nil {
		t.Fatalf("decode exact view golden: %v", err)
	}
	if err := ValidateViewAgainstRequest(want[:len(want)-1], request); err != nil {
		t.Fatalf("validate view against request: %v", err)
	}
}

func TestRequestWireRejectsNonCanonicalFraming(t *testing.T) {
	request := fixture(t, "legacy-governance-read-import-request-v1.json")
	cases := [][]byte{
		request[:len(request)-1],
		append(append([]byte{}, request...), '\n'),
		append([]byte{' '}, request...),
		bytes.Replace(request, []byte(`{"api_version":`),
			[]byte(`{"api_version":"duplicate","api_version":`), 1),
	}
	for index, raw := range cases {
		if _, err := DecodeRequest(raw); err == nil {
			t.Fatalf("case %d: expected rejection", index)
		}
	}
}

func TestProjectionHasNoAuthorityOrWinnerSemantics(t *testing.T) {
	projected, err := Project(fixture(t, "legacy-governance-read-import-request-v1.json"))
	if err != nil {
		t.Fatal(err)
	}
	value, err := parseStrictJSON(projected, maxViewBytes, false)
	if err != nil {
		t.Fatal(err)
	}
	view := value.(map[string]any)
	for _, rawCandidate := range view["candidates"].([]any) {
		candidate := rawCandidate.(map[string]any)
		if candidate["authority"] != nil || candidate["current"] != false ||
			candidate["hardness"] != "none" || candidate["instruction_allowed"] != false ||
			candidate["trust_state"] != "unverified_legacy" {
			t.Fatalf("candidate gained forbidden authority semantics: %#v", candidate)
		}
	}
	for key, value := range view["attestations"].(map[string]any) {
		if value != false {
			t.Fatalf("attestation %s is not false", key)
		}
	}
}

func TestSuccessMarkerAndThirteenFalseSemanticsAreFrozen(t *testing.T) {
	path := filepath.Join("..", "..", "..", "docs", "contracts",
		"legacy-governance-read-import-v1.schema.json")
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var schema map[string]any
	if err := json.Unmarshal(raw, &schema); err != nil {
		t.Fatal(err)
	}
	metadata := schema["x-forgeos-authority-semantics"].(map[string]any)
	if metadata["positive_result"] != successMarker {
		t.Fatal("Go success marker differs from the frozen Schema marker")
	}
	attestations := metadata["attestations"].(map[string]any)
	if len(attestations) != len(attestationFields) || len(attestations) != 13 {
		t.Fatalf("Schema metadata has %d attestations", len(attestations))
	}
	for _, field := range attestationFields {
		if attestations[field] != false {
			t.Fatalf("Schema metadata attestation %s is not false", field)
		}
	}
}

func TestViewRejectsAuthorityAndRawByteTampering(t *testing.T) {
	golden := fixture(t, "legacy-governance-read-import-view-v1.json")
	golden = golden[:len(golden)-1]
	cases := [][]byte{
		bytes.Replace(golden, []byte(`"current":false`), []byte(`"current":true`), 1),
		bytes.Replace(golden, []byte(`"raw_byte_count":139`), []byte(`"raw_byte_count":138`), 1),
		append(append([]byte{}, golden...), '\n'),
	}
	for index, raw := range cases {
		if _, err := DecodeView(raw); err == nil {
			t.Fatalf("case %d: expected view rejection", index)
		}
	}
}

func TestConfidenceRangeIsExactWithoutFloatingPoint(t *testing.T) {
	valid := []string{"0", "-0", "0e999999999999999999999", "1", "1.000000", "1e-7"}
	invalid := []string{"-1e-999999999999999999999", "1.0000001", "2", "1e1"}
	for _, raw := range valid {
		if !confidenceInRange(raw) {
			t.Errorf("expected %q to be in range", raw)
		}
	}
	for _, raw := range invalid {
		if confidenceInRange(raw) {
			t.Errorf("expected %q to be outside range", raw)
		}
	}
	exact := "0e+" + strings.Repeat("0", 125)
	if _, err := Project(testRequest(t, []testSource{{memoryKind, "memory", confidenceMemory(exact)}})); err != nil {
		t.Fatalf("exact confidence lexeme bound rejected: %v", err)
	}
	if _, err := Project(testRequest(t, []testSource{{memoryKind, "memory", confidenceMemory(exact + "0")}})); err == nil {
		t.Fatal("N+1 confidence lexeme bound accepted")
	}
}

func TestProjectConfidenceExtremeExponentParity(t *testing.T) {
	valid := []string{"0e999999999999999999999", "1e-999999999999999999999",
		"0.01e-9223372036854775808"}
	invalid := []string{"-1e-999999999999999999999", "1e999999999999999999999",
		"1e9223372036854775807", "NaN", "1."}
	for _, lexeme := range valid {
		request := testRequest(t, []testSource{{memoryKind, "memory", confidenceMemory(lexeme)}})
		if _, err := Project(request); err != nil {
			t.Errorf("valid extreme confidence %q rejected: %v", lexeme, err)
		}
	}
	for _, lexeme := range invalid {
		request := testRequest(t, []testSource{{memoryKind, "memory", confidenceMemory(lexeme)}})
		if _, err := Project(request); err == nil {
			t.Errorf("invalid extreme confidence %q accepted", lexeme)
		}
	}
}

func TestProjectRejectsHugeMemoryIntegerWithoutPanicking(t *testing.T) {
	memory := []byte(`{"kind":"gap","topic":"t","detail":"d","iteration":` +
		strings.Repeat("1", 5000) + `,"created_at_unix":2}` + "\n")
	request := testRequest(t, []testSource{{memoryKind, "memory", memory}})
	if _, err := Project(request); err == nil {
		t.Fatal("memory integer beyond int64 was accepted")
	}
}

func TestRequestSourceRefADRSizeAndCountBoundaries(t *testing.T) {
	exactRef := strings.Repeat("r", maxSourceRefBytes)
	if _, err := Project(testRequest(t, []testSource{{adrKind, exactRef, []byte("x\n")}})); err != nil {
		t.Fatalf("exact ref bound rejected: %v", err)
	}
	if _, err := Project(testRequest(t, []testSource{{adrKind, exactRef + "r", []byte("x\n")}})); err == nil {
		t.Fatal("N+1 ref bound accepted")
	}
	exactADR := append(bytes.Repeat([]byte{'x'}, maxADRBytes-1), '\n')
	if _, err := Project(testRequest(t, []testSource{{adrKind, "exact", exactADR}})); err != nil {
		t.Fatalf("exact ADR bound rejected: %v", err)
	}
	tooLarge := append(bytes.Repeat([]byte{'x'}, maxADRBytes), '\n')
	if _, err := Project(testRequest(t, []testSource{{adrKind, "large", tooLarge}})); err == nil {
		t.Fatal("N+1 ADR bound accepted")
	}
}

func TestRequestADRCountBoundary(t *testing.T) {
	sources := make([]testSource, 0, maxADRSources+1)
	for index := 0; index < maxADRSources; index++ {
		sources = append(sources, testSource{adrKind, fmt.Sprintf("adr-%03d", index), []byte("x\n")})
	}
	if _, err := Project(testRequest(t, sources)); err != nil {
		t.Fatalf("exact ADR count rejected: %v", err)
	}
	sources = append(sources, testSource{adrKind, "adr-256", []byte("x\n")})
	if _, err := Project(testRequest(t, sources)); err == nil {
		t.Fatal("N+1 ADR count accepted")
	}
}

func TestStandaloneViewRejectsSelfConsistentIllegalRefAndADRSize(t *testing.T) {
	golden := fixture(t, "legacy-governance-read-import-view-v1.json")
	value, err := parseStrictJSON(golden[:len(golden)-1], maxViewBytes, false)
	if err != nil {
		t.Fatal(err)
	}
	view := value.(map[string]any)
	candidate := view["candidates"].([]any)[4].(map[string]any)
	descriptor := view["sources"].([]any)[1].(map[string]any)
	illegal := candidate["source_ref"].(string) + strings.Repeat("x", maxSourceRefBytes)
	candidate["source_ref"], candidate["document_name"], descriptor["source_ref"] = illegal, illegal, illegal
	resealTestCandidate(t, candidate)
	if _, err := DecodeView(resealTestView(t, view)); err == nil {
		t.Fatal("self-consistent N+1 source ref view accepted")
	}

	value, _ = parseStrictJSON(golden[:len(golden)-1], maxViewBytes, false)
	view = value.(map[string]any)
	candidate = view["candidates"].([]any)[4].(map[string]any)
	descriptor = view["sources"].([]any)[1].(map[string]any)
	raw := append(bytes.Repeat([]byte{'x'}, maxADRBytes), '\n')
	candidate["raw_byte_count"] = int64(len(raw))
	candidate["raw_bytes_base64url"], candidate["raw_sha256"] = encodeBase64URL(raw), shaBytes(raw)
	descriptor["byte_count"], descriptor["content_sha256"] = int64(len(raw)), shaBytes(raw)
	resealTestCandidate(t, candidate)
	if _, err := DecodeView(resealTestView(t, view)); err == nil {
		t.Fatal("self-consistent N+1 ADR view accepted")
	}
}

func TestStandaloneViewRejectsNonStringSourceKinds(t *testing.T) {
	golden := fixture(t, "legacy-governance-read-import-view-v1.json")
	for _, target := range []string{"candidate", "descriptor"} {
		value, err := parseStrictJSON(golden[:len(golden)-1], maxViewBytes, false)
		if err != nil {
			t.Fatal(err)
		}
		view := value.(map[string]any)
		if target == "candidate" {
			candidate := view["candidates"].([]any)[0].(map[string]any)
			candidate["source_kind"] = []any{}
			resealTestCandidate(t, candidate)
		} else {
			descriptor := view["sources"].([]any)[0].(map[string]any)
			descriptor["source_kind"] = map[string]any{}
		}
		if _, err := DecodeView(resealTestView(t, view)); err == nil {
			t.Fatalf("non-string %s source kind was accepted", target)
		}
	}
}

func TestStandaloneViewMemoryCandidateCountBoundary(t *testing.T) {
	if _, err := DecodeView(memoryOnlyView(t, maxMemoryEntries)); err != nil {
		t.Fatalf("memory candidate count N rejected: %v", err)
	}
	if _, err := DecodeView(memoryOnlyView(t, maxMemoryEntries+1)); err == nil {
		t.Fatal("memory candidate count N+1 accepted")
	}
}
