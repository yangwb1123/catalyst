package legacygovernanceimportcontract

import (
	"bytes"
	"fmt"
	"slices"
	"unicode/utf8"
)

var (
	requestFields = []string{
		"api_version", "binding", "canonicalization", "kind", "request_sha256", "sources",
	}
	bindingFields = []string{"project_id", "source_revision", "source_tree_sha256"}
	sourceFields  = []string{
		"byte_count", "content_base64url", "content_sha256", "source_kind", "source_ref",
	}
	descriptorFields = []string{"byte_count", "content_sha256", "source_kind", "source_ref"}
)

type decodedRequest struct {
	value      map[string]any
	rawSources [][]byte
}

// DecodeRequest validates one compact canonical request JSON document followed by one LF.
func DecodeRequest(raw []byte) (map[string]any, error) {
	decoded, err := decodeRequest(raw)
	if err != nil {
		return nil, err
	}
	return decoded.value, nil
}

func decodeRequest(raw []byte) (*decodedRequest, error) {
	if len(raw) < 2 || len(raw) > maxRequestBytes || raw[len(raw)-1] != '\n' ||
		raw[len(raw)-2] == '\n' {
		return nil, fmt.Errorf("request wire must end in exactly one LF")
	}
	body := raw[:len(raw)-1]
	value, err := parseStrictJSON(body, maxRequestBytes-1, false)
	if err != nil {
		return nil, fmt.Errorf("request JSON: %w", err)
	}
	canonical, err := canonicalJSON(value, maxRequestBytes-1, "request")
	if err != nil || !bytes.Equal(canonical, body) {
		return nil, fmt.Errorf("request is not exact compact canonical JSON")
	}
	request, err := exactFields(value, requestFields, "request")
	if err != nil {
		return nil, err
	}
	if request["api_version"] != requestAPI || request["kind"] != requestKind ||
		request["canonicalization"] != canonicalization {
		return nil, fmt.Errorf("request frozen identity constants do not match")
	}
	if err := validateBinding(request["binding"]); err != nil {
		return nil, err
	}
	rawSources, err := validateSources(request["sources"])
	if err != nil {
		return nil, err
	}
	digest, err := requireDigest(request["request_sha256"], "request_sha256")
	if err != nil {
		return nil, err
	}
	expected, err := selfDigest(requestDomain, request, "request_sha256",
		maxRequestBytes, "request")
	if err != nil || digest != expected {
		return nil, fmt.Errorf("request_sha256 does not match canonical request")
	}
	return &decodedRequest{value: request, rawSources: rawSources}, nil
}

func validateBinding(value any) error {
	binding, err := exactFields(value, bindingFields, "binding")
	if err != nil {
		return err
	}
	for _, field := range []string{"project_id", "source_revision"} {
		text, err := stringValue(binding, field, "binding", maxBindingBytes)
		if err != nil || !identifierPattern.MatchString(text) {
			return fmt.Errorf("binding.%s is not a closed ASCII identifier", field)
		}
	}
	_, err = requireDigest(binding["source_tree_sha256"], "source_tree_sha256")
	return err
}

func validateSources(value any) ([][]byte, error) {
	sources, ok := value.([]any)
	if !ok || len(sources) < 1 || len(sources) > maxADRSources+1 {
		return nil, fmt.Errorf("sources cardinality must be 1..257")
	}
	raws := make([][]byte, 0, len(sources))
	keys := make([]string, 0, len(sources))
	memoryCount, adrCount, total := 0, 0, 0
	for index, value := range sources {
		source, raw, key, err := validateSource(value, index)
		if err != nil {
			return nil, err
		}
		if source["source_kind"] == memoryKind {
			memoryCount++
		} else {
			adrCount++
		}
		total += len(raw)
		raws = append(raws, raw)
		keys = append(keys, key)
	}
	if !slices.IsSorted(keys) || adjacentDuplicate(keys) || memoryCount > 1 ||
		adrCount > maxADRSources || total > maxRawBytes {
		return nil, fmt.Errorf("sources violate order, uniqueness, cardinality, or aggregate bound")
	}
	return raws, nil
}

func validateSource(value any, index int) (map[string]any, []byte, string, error) {
	label := fmt.Sprintf("sources[%d]", index)
	source, err := exactFields(value, sourceFields, label)
	if err != nil {
		return nil, nil, "", err
	}
	kind, err := stringValue(source, "source_kind", label, 64)
	if err != nil || (kind != memoryKind && kind != adrKind) {
		return nil, nil, "", fmt.Errorf("%s source_kind is unsupported", label)
	}
	ref, err := stringValue(source, "source_ref", label, maxSourceRefBytes)
	if err != nil {
		return nil, nil, "", err
	}
	raw, err := decodeBase64URL(source["content_base64url"], label+" content")
	if err != nil {
		return nil, nil, "", err
	}
	count, ok := source["byte_count"].(int64)
	digest, digestErr := requireDigest(source["content_sha256"], label+" content_sha256")
	if !ok || count != int64(len(raw)) || digestErr != nil || digest != shaBytes(raw) {
		return nil, nil, "", fmt.Errorf("%s byte count or digest does not match", label)
	}
	maximum := maxADRBytes
	if kind == memoryKind {
		maximum = maxMemoryBytes
	}
	if len(raw) == 0 || len(raw) > maximum || !utf8.Valid(raw) ||
		bytes.ContainsRune(raw, '\r') || raw[len(raw)-1] != '\n' {
		return nil, nil, "", fmt.Errorf("%s has invalid UTF-8/LF framing or size", label)
	}
	return source, raw, kind + "\x00" + ref, nil
}

func adjacentDuplicate(values []string) bool {
	for index := 1; index < len(values); index++ {
		if values[index] == values[index-1] {
			return true
		}
	}
	return false
}

func sourceDescriptors(request map[string]any) []any {
	sources := request["sources"].([]any)
	result := make([]any, 0, len(sources))
	for _, value := range sources {
		source := value.(map[string]any)
		descriptor := make(map[string]any, len(descriptorFields))
		for _, field := range descriptorFields {
			descriptor[field] = source[field]
		}
		result = append(result, descriptor)
	}
	return result
}
