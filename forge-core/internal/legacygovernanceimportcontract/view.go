package legacygovernanceimportcontract

import (
	"bytes"
	"fmt"
	"reflect"
	"sort"
	"unicode/utf8"
)

var (
	viewFields = []string{
		"api_version", "attestations", "binding", "candidates", "canonicalization",
		"conflict_sets", "declared_supersessions", "kind", "request_sha256", "result",
		"source_set_sha256", "sources", "view_sha256",
	}
	attestationFields = []string{
		"acceptance", "authority", "confidence_interpretation", "conflict_resolution",
		"currentness", "instruction_eligibility", "persistence", "runtime_effect",
		"source_authentication", "source_completeness", "status_interpretation", "truth",
		"winner_selection",
	}
	commonCandidateFields = []string{
		"authority", "candidate_id", "candidate_sha256", "current", "hardness",
		"instruction_allowed", "ordinal", "raw_byte_count", "raw_bytes_base64url",
		"raw_sha256", "request_sha256", "source_kind", "source_ref", "trust_state",
	}
	memoryCandidateFields = appendCopy(commonCandidateFields,
		"confidence", "created_at_unix", "declared_kind", "declared_source",
		"declared_supersedes", "declared_topic", "detail", "iteration", "legacy_format")
	adrCandidateFields = appendCopy(commonCandidateFields, "document_name", "parsing")
)

func appendCopy(base []string, additions ...string) []string {
	result := append([]string{}, base...)
	return append(result, additions...)
}

// DecodeView validates an exact canonical view document without trailing framing.
func DecodeView(raw []byte) (map[string]any, error) {
	value, err := parseStrictJSON(raw, maxViewBytes, false)
	if err != nil {
		return nil, fmt.Errorf("view JSON: %w", err)
	}
	canonical, err := canonicalJSON(value, maxViewBytes, "view")
	if err != nil || !bytes.Equal(canonical, raw) {
		return nil, fmt.Errorf("view is not exact compact canonical JSON")
	}
	view, err := exactFields(value, viewFields, "view")
	if err != nil {
		return nil, err
	}
	if view["api_version"] != viewAPI || view["kind"] != viewKind ||
		view["canonicalization"] != canonicalization || view["result"] != result {
		return nil, fmt.Errorf("view frozen identity constants do not match")
	}
	requestSHA, err := requireDigest(view["request_sha256"], "view request_sha256")
	if err != nil {
		return nil, err
	}
	if err := validateBinding(view["binding"]); err != nil {
		return nil, err
	}
	if err := validateAttestations(view["attestations"]); err != nil {
		return nil, err
	}
	candidates, err := validateCandidates(view["candidates"], requestSHA)
	if err != nil {
		return nil, err
	}
	if err := validateViewRelations(view, candidates, requestSHA); err != nil {
		return nil, err
	}
	if _, err := reconstructSources(view, candidates); err != nil {
		return nil, err
	}
	want, err := selfDigest(viewDomain, view, "view_sha256", maxViewBytes, "view")
	got, digestErr := requireDigest(view["view_sha256"], "view_sha256")
	if err != nil || digestErr != nil || want != got {
		return nil, fmt.Errorf("view_sha256 does not match")
	}
	return view, nil
}

func validateAttestations(value any) error {
	attestations, err := exactFields(value, attestationFields, "attestations")
	if err != nil {
		return err
	}
	for _, field := range attestationFields {
		if attestations[field] != false {
			return fmt.Errorf("attestation %s must be false", field)
		}
	}
	return nil
}

func validateCandidates(value any, requestSHA string) ([]any, error) {
	candidates, ok := value.([]any)
	if !ok || len(candidates) == 0 || len(candidates) > maxArrayItems {
		return nil, fmt.Errorf("candidates must be a bounded nonempty array")
	}
	var previousKind, previousRef string
	var previousOrdinal int64
	seenIDs := make(map[string]bool, len(candidates))
	for index, value := range candidates {
		candidate, err := validateCandidate(value, index, requestSHA)
		if err != nil {
			return nil, err
		}
		kind := candidate["source_kind"].(string)
		ref := candidate["source_ref"].(string)
		ordinal := candidate["ordinal"].(int64)
		if index > 0 && (kind < previousKind ||
			(kind == previousKind && (ref < previousRef ||
				(ref == previousRef && ordinal <= previousOrdinal)))) {
			return nil, fmt.Errorf("candidates are not uniquely source/ordinal ordered")
		}
		previousKind, previousRef, previousOrdinal = kind, ref, ordinal
		id := candidate["candidate_id"].(string)
		if seenIDs[id] {
			return nil, fmt.Errorf("candidate IDs must be unique")
		}
		seenIDs[id] = true
	}
	return candidates, nil
}

func validateCandidate(value any, index int, requestSHA string) (map[string]any, error) {
	candidate, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("candidate %d must be an object", index)
	}
	kind, ok := candidate["source_kind"].(string)
	fields := adrCandidateFields
	if kind == memoryKind {
		fields = memoryCandidateFields
	} else if !ok || kind != adrKind {
		return nil, fmt.Errorf("candidate %d source_kind is unsupported", index)
	}
	if _, err := exactFields(candidate, fields, fmt.Sprintf("candidate %d", index)); err != nil {
		return nil, err
	}
	if candidate["request_sha256"] != requestSHA || candidate["authority"] != nil ||
		candidate["current"] != false || candidate["hardness"] != "none" ||
		candidate["instruction_allowed"] != false || candidate["trust_state"] != "unverified_legacy" {
		return nil, fmt.Errorf("candidate %d authority/trust constants do not match", index)
	}
	ordinal, ok := candidate["ordinal"].(int64)
	if !ok || ordinal < 1 {
		return nil, fmt.Errorf("candidate %d ordinal must be positive", index)
	}
	if _, err := stringValue(candidate, "source_ref", "candidate", maxSourceRefBytes); err != nil {
		return nil, err
	}
	raw, err := decodeBase64URL(candidate["raw_bytes_base64url"], "candidate raw bytes")
	if err != nil {
		return nil, err
	}
	count, countOK := candidate["raw_byte_count"].(int64)
	rawSHA, shaErr := requireDigest(candidate["raw_sha256"], "candidate raw_sha256")
	if !countOK || count != int64(len(raw)) || shaErr != nil || rawSHA != shaBytes(raw) {
		return nil, fmt.Errorf("candidate %d raw byte pins do not match", index)
	}
	if err := validateCandidateProjection(candidate, raw, kind, ordinal); err != nil {
		return nil, err
	}
	if err := validateCandidateIdentity(candidate, kind, ordinal, requestSHA, index); err != nil {
		return nil, err
	}
	return candidate, nil
}

func validateCandidateIdentity(candidate map[string]any, kind string,
	ordinal int64, requestSHA string, index int) error {
	locator := map[string]any{
		"ordinal": ordinal, "request_sha256": requestSHA,
		"source_kind": kind, "source_ref": candidate["source_ref"],
	}
	id, err := digestValue(candidateIDDomain, locator, maxRequestBytes, "candidate locator")
	if err != nil || candidate["candidate_id"] != id {
		return fmt.Errorf("candidate %d ID does not match request-bound locator", index)
	}
	want, err := selfDigest(candidateDomain, candidate, "candidate_sha256",
		maxViewBytes, "candidate")
	got, digestErr := requireDigest(candidate["candidate_sha256"], "candidate_sha256")
	if err != nil || digestErr != nil || want != got {
		return fmt.Errorf("candidate %d digest does not match", index)
	}
	return nil
}

func validateCandidateProjection(candidate map[string]any, raw []byte,
	kind string, ordinal int64) error {
	if !utf8.Valid(raw) {
		return fmt.Errorf("candidate raw bytes are not strict UTF-8")
	}
	if kind == memoryKind {
		if ordinal > maxMemoryEntries {
			return fmt.Errorf("memory candidate ordinal exceeds its entry bound")
		}
		if bytes.ContainsAny(raw, "\r\n") {
			return fmt.Errorf("memory candidate is not one unframed line")
		}
		projected, err := parseMemoryLine(raw, int(ordinal))
		if err != nil {
			return err
		}
		for field, value := range projected {
			if !reflect.DeepEqual(candidate[field], value) {
				return fmt.Errorf("memory candidate field %s does not match raw line", field)
			}
		}
		return nil
	}
	if ordinal != 1 || candidate["document_name"] != candidate["source_ref"] ||
		candidate["parsing"] != "not_performed" || bytes.ContainsRune(raw, '\r') ||
		len(raw) == 0 || raw[len(raw)-1] != '\n' || len(raw) > maxADRBytes {
		return fmt.Errorf("ADR candidate framing or no-parse constants do not match")
	}
	return nil
}

func validateViewRelations(view map[string]any, candidates []any, requestSHA string) error {
	conflicts, err := conflictSets(requestSHA, candidates)
	if err != nil || !reflect.DeepEqual(view["conflict_sets"], conflicts) {
		return fmt.Errorf("conflict sets are not the complete deterministic grouping")
	}
	declarations, err := supersessions(requestSHA, candidates)
	if err != nil || !reflect.DeepEqual(view["declared_supersessions"], declarations) {
		return fmt.Errorf("supersessions do not preserve every declaration")
	}
	return nil
}

func reconstructSources(view map[string]any, candidates []any) (map[string][]byte, error) {
	descriptors, ok := view["sources"].([]any)
	if !ok || len(descriptors) == 0 || len(descriptors) > maxADRSources+1 {
		return nil, fmt.Errorf("view sources must be a bounded nonempty array")
	}
	sourceSHA, err := digestValue(sourceSetDomain, descriptors, maxViewBytes,
		"source descriptor set")
	if err != nil || view["source_set_sha256"] != sourceSHA {
		return nil, fmt.Errorf("source_set_sha256 does not match")
	}
	grouped := make(map[string][]map[string]any)
	for _, value := range candidates {
		candidate := value.(map[string]any)
		key := candidate["source_kind"].(string) + "\x00" + candidate["source_ref"].(string)
		grouped[key] = append(grouped[key], candidate)
	}
	result := make(map[string][]byte)
	keys := make([]string, 0, len(descriptors))
	memoryCount, adrCount, total := 0, 0, 0
	for index, value := range descriptors {
		key, kind, raw, err := reconstructDescriptor(value, index, grouped)
		if err != nil {
			return nil, err
		}
		keys = append(keys, key)
		delete(grouped, key)
		if kind == memoryKind {
			memoryCount++
		} else {
			adrCount++
		}
		total += len(raw)
		result[key] = raw
	}
	if !sort.StringsAreSorted(keys) || adjacentDuplicate(keys) || len(grouped) != 0 ||
		memoryCount > 1 || adrCount > maxADRSources || total > maxRawBytes {
		return nil, fmt.Errorf("view sources are unordered, duplicate, or incomplete")
	}
	return result, nil
}

func reconstructDescriptor(value any, index int,
	grouped map[string][]map[string]any) (string, string, []byte, error) {
	descriptor, err := exactFields(value, descriptorFields, "source descriptor")
	if err != nil {
		return "", "", nil, err
	}
	kind, kindOK := descriptor["source_kind"].(string)
	ref, refOK := descriptor["source_ref"].(string)
	if !kindOK || !refOK || (kind != memoryKind && kind != adrKind) ||
		validateWireString(ref, maxSourceRefBytes, true) != nil {
		return "", "", nil, fmt.Errorf("source descriptor %d identity is invalid", index)
	}
	key := kind + "\x00" + ref
	raw, err := reconstructOne(kind, grouped[key])
	if err != nil {
		return "", "", nil, err
	}
	count, countOK := descriptor["byte_count"].(int64)
	digest, digestErr := requireDigest(descriptor["content_sha256"], "source content_sha256")
	maximum := maxADRBytes
	if kind == memoryKind {
		maximum = maxMemoryBytes
	}
	if !countOK || count != int64(len(raw)) || digestErr != nil ||
		digest != shaBytes(raw) || len(raw) > maximum {
		return "", "", nil, fmt.Errorf("reconstructed source %d violates descriptor or bound", index)
	}
	return key, kind, raw, nil
}

func reconstructOne(kind string, members []map[string]any) ([]byte, error) {
	if len(members) == 0 || (kind == memoryKind && len(members) > maxMemoryEntries) ||
		(kind == adrKind && len(members) != 1) {
		return nil, fmt.Errorf("source candidate cardinality is invalid")
	}
	var raw []byte
	for index, candidate := range members {
		if candidate["ordinal"] != int64(index+1) {
			return nil, fmt.Errorf("source candidate ordinals are not contiguous")
		}
		part, err := decodeBase64URL(candidate["raw_bytes_base64url"], "candidate raw")
		if err != nil {
			return nil, err
		}
		raw = append(raw, part...)
		if kind == memoryKind {
			raw = append(raw, '\n')
		}
	}
	return raw, nil
}

// ValidateViewAgainstRequest proves exact byte reconstruction and deterministic projection.
func ValidateViewAgainstRequest(viewDocument, requestWire []byte) error {
	request, err := decodeRequest(requestWire)
	if err != nil {
		return err
	}
	view, err := DecodeView(viewDocument)
	if err != nil {
		return err
	}
	if view["request_sha256"] != request.value["request_sha256"] ||
		!reflect.DeepEqual(view["binding"], request.value["binding"]) {
		return fmt.Errorf("view binding does not match request")
	}
	candidates := view["candidates"].([]any)
	reconstructed, err := reconstructSources(view, candidates)
	if err != nil {
		return err
	}
	for index, value := range request.value["sources"].([]any) {
		source := value.(map[string]any)
		key := source["source_kind"].(string) + "\x00" + source["source_ref"].(string)
		if !bytes.Equal(reconstructed[key], request.rawSources[index]) {
			return fmt.Errorf("view does not reconstruct exact request source %d", index)
		}
	}
	expected, err := Project(requestWire)
	if err != nil || !bytes.Equal(expected, viewDocument) {
		return fmt.Errorf("view is not the unique deterministic projection")
	}
	return nil
}
