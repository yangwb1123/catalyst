package graphsnapshot

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"strconv"
	"unicode"
	"unicode/utf8"
)

const (
	maxJSONDepth        = 16
	maxJSONObjectFields = 64
	defaultArrayLimit   = 65_536
)

type jsonShapeLimits struct {
	discriminatorOnly    bool
	maxAggregateLocators int
	maxEdges             int
	maxNodes             int
	maxReasonCodes       int
	sourceLocators       int
}

func validateEnvelopeJSONShape(raw []byte) error {
	return validateEnvelopeJSONShapeForProfile(raw, legacyProfile)
}

func validateEnvelopeJSONShapeForProfile(raw []byte, profile projectionProfile) error {
	return validateJSONShapeWithLimits(raw, maxEnvelopeBytes, &jsonShapeLimits{
		maxAggregateLocators: profile.maxAggregateLocators,
		maxEdges:             profile.maxEdges, maxNodes: profile.maxNodes,
		maxReasonCodes: profile.maxReasonCodes,
	})
}

func validateJSONShape(raw []byte, maxBytes int) error {
	return validateJSONShapeWithLimits(raw, maxBytes, legacyShapeLimits())
}

func validateDiscriminatorJSONShape(raw []byte, maxBytes int) error {
	return validateJSONShapeWithLimits(raw, maxBytes, &jsonShapeLimits{
		discriminatorOnly: true, maxEdges: maxTestSourceEdges,
		maxNodes: maxTestSourceNodes, maxReasonCodes: 24,
	})
}

func legacyShapeLimits() *jsonShapeLimits {
	return &jsonShapeLimits{
		maxAggregateLocators: maxAggregateLocators,
		maxEdges:             maxEdges, maxNodes: maxNodes, maxReasonCodes: 20,
	}
}

func validateJSONShapeWithLimits(
	raw []byte, maxBytes int, limits *jsonShapeLimits,
) error {
	if len(raw) == 0 || len(raw) > maxBytes || !utf8.Valid(raw) {
		return fmt.Errorf("envelope is empty, oversized, or invalid UTF-8")
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	if err := consumeJSONValue(decoder, nil, 1, limits); err != nil {
		return err
	}
	if _, err := decoder.Token(); err != io.EOF {
		return fmt.Errorf("envelope JSON has trailing data")
	}
	return nil
}

func consumeJSONValue(
	decoder *json.Decoder, path []string, depth int, limits *jsonShapeLimits,
) error {
	if depth > maxJSONDepth {
		return fmt.Errorf("JSON depth exceeds %d", maxJSONDepth)
	}
	token, err := decoder.Token()
	if err != nil {
		return err
	}
	if delimiter, ok := token.(json.Delim); ok {
		switch delimiter {
		case '{':
			return consumeJSONObject(decoder, path, depth, limits)
		case '[':
			return consumeJSONArray(decoder, path, depth, limits)
		default:
			return fmt.Errorf("unexpected closing delimiter")
		}
	}
	return validateJSONScalar(token, path, limits)
}

func consumeJSONObject(
	decoder *json.Decoder, path []string, depth int, limits *jsonShapeLimits,
) error {
	seen := map[string]struct{}{}
	for count := 0; decoder.More(); count++ {
		keyToken, err := decoder.Token()
		key, ok := keyToken.(string)
		if err != nil || !ok || !validJSONKey(key) || count >= maxJSONObjectFields {
			return fmt.Errorf("invalid or excessive object member")
		}
		if _, exists := seen[key]; exists {
			return fmt.Errorf("duplicate object key %q", key)
		}
		seen[key] = struct{}{}
		if err := consumeJSONValue(decoder, appendPath(path, key), depth+1, limits); err != nil {
			return err
		}
	}
	return requireDelimiter(decoder, '}')
}

func consumeJSONArray(
	decoder *json.Decoder, path []string, depth int, limits *jsonShapeLimits,
) error {
	limit := arrayLimit(path, limits)
	if limits.discriminatorOnly {
		limit = limits.maxEdges
	}
	for count := 0; decoder.More(); count++ {
		if count >= limit {
			return fmt.Errorf("JSON array exceeds %d items", limit)
		}
		if !limits.discriminatorOnly && isSnapshotSourceLocatorPath(path) {
			limits.sourceLocators++
			if limits.sourceLocators > limits.maxAggregateLocators {
				return fmt.Errorf("aggregate source locators exceed %d", limits.maxAggregateLocators)
			}
		}
		if err := consumeJSONValue(decoder, path, depth+1, limits); err != nil {
			return err
		}
	}
	return requireDelimiter(decoder, ']')
}

func isSnapshotSourceLocatorPath(path []string) bool {
	return len(path) == 3 && path[0] == "snapshot" &&
		(path[1] == "nodes" || path[1] == "edges" ||
			path[1] == "unresolved_nodes" || path[1] == "unresolved_edges") &&
		path[2] == "source_locators"
}

func validateJSONScalar(value any, path []string, limits *jsonShapeLimits) error {
	switch typed := value.(type) {
	case nil, bool:
		return nil
	case string:
		if limits.discriminatorOnly {
			if len(typed) > maxBase64Bytes || containsForbiddenText(typed) {
				return fmt.Errorf("string violates discriminator safety profile")
			}
			return nil
		}
		if !boundedJSONString(typed, path) {
			return fmt.Errorf("string violates bounded text profile")
		}
		return nil
	case json.Number:
		parsed, err := strconv.ParseInt(string(typed), 10, 64)
		if err != nil || strconv.FormatInt(parsed, 10) != string(typed) {
			return fmt.Errorf("number is not a signed int64")
		}
		return nil
	default:
		return fmt.Errorf("unsupported JSON scalar")
	}
}

func containsForbiddenText(value string) bool {
	for _, character := range value {
		if unicode.Is(unicode.Cc, character) || forbiddenDirectional(character) {
			return true
		}
	}
	return false
}

func boundedJSONString(value string, path []string) bool {
	field := ""
	if len(path) != 0 {
		field = path[len(path)-1]
	}
	if field == "graph_observation_base64url" {
		if value == "" || len(value) > maxBase64Bytes {
			return false
		}
		for _, character := range []byte(value) {
			letter := character >= 'A' && character <= 'Z' || character >= 'a' && character <= 'z'
			digit := character >= '0' && character <= '9'
			if !letter && !digit && character != '_' && character != '-' {
				return false
			}
		}
		return true
	}
	if field == "observer_run_id" || field == "project_id" || field == "run_id" {
		return len(value) <= 160
	}
	return value == "" || validBoundedText(value)
}

func validJSONKey(value string) bool {
	if value == "" || !validBoundedText(value) || value[0] < 'a' || value[0] > 'z' {
		return false
	}
	for _, character := range []byte(value) {
		if character != '_' && (character < 'a' || character > 'z') &&
			(character < '0' || character > '9') {
			return false
		}
	}
	return true
}

func requireDelimiter(decoder *json.Decoder, expected json.Delim) error {
	token, err := decoder.Token()
	if err != nil || token != expected {
		return fmt.Errorf("JSON delimiter is malformed")
	}
	return nil
}

func appendPath(path []string, key string) []string {
	result := make([]string, len(path)+1)
	copy(result, path)
	result[len(path)] = key
	return result
}

func pathEquals(path []string, values ...string) bool {
	if len(path) != len(values) {
		return false
	}
	for index := range path {
		if path[index] != values[index] {
			return false
		}
	}
	return true
}

func arrayLimit(path []string, limits *jsonShapeLimits) int {
	if pathEquals(path, "snapshot", "edges") {
		return limits.maxEdges
	}
	if pathEquals(path, "snapshot", "nodes") {
		return limits.maxNodes
	}
	if pathEquals(path, "snapshot", "unresolved_nodes") {
		return maxUnresolvedNodes
	}
	if len(path) == 0 {
		return defaultArrayLimit
	}
	return fieldArrayLimit(path[len(path)-1], limits)
}

func fieldArrayLimit(field string, limits *jsonShapeLimits) int {
	switch field {
	case "adr_0062_node_crosswalk", "source_locators", "target_node_ids":
		return maxLocators
	case "category_axes":
		return 7
	case "claim_record_ids", "evidence_record_ids", "owner_node_ids":
		return 0
	case "extractor_sha256s", "extractors", "source_ids", "sources":
		return 1
	case "reason_codes":
		return limits.maxReasonCodes
	case "surfaces":
		return 11
	case "qualified_name_components":
		return 3
	case "candidate_qualified_name_components":
		return 2
	default:
		return defaultArrayLimit
	}
}
