package capabilityregistry

import (
	"strings"
	"testing"
)

func TestStrictJSONRejectsCanonicalAndUnicodeDrift(t *testing.T) {
	cases := [][]byte{
		[]byte(`{"a":1,"a":1}`),
		[]byte(`{"a":1.0}`),
		[]byte(`{"a":1e0}`),
		[]byte(`{"a":01}`),
		[]byte(`{"a":9223372036854775808}`),
		[]byte("{\"a\":\"line\\n\"}"),
		[]byte("{\"a\":\"\u202e\"}"),
		[]byte(`{"A":1}`),
		append([]byte(`{"a":"`), 0xff, '"', '}'),
	}
	for index, raw := range cases {
		if _, err := parseStrictJSON(raw, maxRegistryBytes); err == nil {
			t.Fatalf("case %d unexpectedly accepted", index)
		}
	}
}

func TestStrictJSONDepthAndDocumentBounds(t *testing.T) {
	within := []byte(strings.Repeat("[", maxJSONDepth-1) + "0" + strings.Repeat("]", maxJSONDepth-1))
	if _, err := parseStrictJSON(within, len(within)); err != nil {
		t.Fatalf("depth within bound: %v", err)
	}
	over := []byte(strings.Repeat("[", maxJSONDepth) + "0" + strings.Repeat("]", maxJSONDepth))
	if _, err := parseStrictJSON(over, len(over)); err == nil {
		t.Fatal("depth over bound accepted")
	}
	if _, err := parseStrictJSON([]byte(`{}`), 1); err == nil {
		t.Fatal("document over byte bound accepted")
	}
}

func TestContentRefsRejectDuplicateLocatorWithDifferentBytes(t *testing.T) {
	first := testContentRef("a.json", "#", strings.Repeat("1", 64), 1)
	second := testContentRef("a.json", "#", strings.Repeat("2", 64), 2)
	items := []any{first, second}
	canonicalSortObjects(t, items)
	if err := validateContentRefs(items, false); err == nil {
		t.Fatal("duplicate path/selector tuple accepted")
	}
}

func TestJSONPointersAreASCIIAndCanonical(t *testing.T) {
	for _, value := range []string{"#", "#/a", "#/a~0b", "#/a~1b"} {
		if !validJSONPointer(value, true) {
			t.Fatalf("valid fragment %q rejected", value)
		}
	}
	for _, value := range []string{"#/é", "#/a~", "#/a~2", "a", "##"} {
		if validJSONPointer(value, true) {
			t.Fatalf("invalid fragment %q accepted", value)
		}
	}
}

func TestPredicateValueRelationIsClosed(t *testing.T) {
	base := map[string]any{"document": "input", "json_pointer": "/x"}
	for _, operator := range []string{"absent", "present"} {
		value := cloneJSON(base).(map[string]any)
		value["operator"], value["value"] = operator, nil
		if err := validatePredicate(value); err != nil {
			t.Fatalf("%s/null: %v", operator, err)
		}
		value["value"] = "x"
		if err := validatePredicate(value); err == nil {
			t.Fatalf("%s/string accepted", operator)
		}
	}
	for _, operator := range []string{"equals", "not_equals"} {
		value := cloneJSON(base).(map[string]any)
		value["operator"], value["value"] = operator, ""
		if err := validatePredicate(value); err != nil {
			t.Fatalf("%s/string: %v", operator, err)
		}
		value["value"] = nil
		if err := validatePredicate(value); err == nil {
			t.Fatalf("%s/null accepted", operator)
		}
	}
	for _, value := range []string{"bad\x00value", "bad\u202evalue"} {
		predicate := cloneJSON(base).(map[string]any)
		predicate["operator"], predicate["value"] = "equals", value
		if err := validatePredicate(predicate); err == nil {
			t.Fatalf("forbidden predicate value %q accepted", value)
		}
	}
}

func TestTypedSelectorsRejectForbiddenScalars(t *testing.T) {
	for _, value := range []string{"#/bad\x00value", "#/bad\u202evalue"} {
		if err := validateSelector(value); err == nil {
			t.Fatalf("forbidden selector %q accepted", value)
		}
	}
}

func TestEffectAndPermissionVocabulariesAreClosed(t *testing.T) {
	if knownEffects([]any{"repo.read", "unknown.effect"}) {
		t.Fatal("unknown effect accepted")
	}
	valid := map[string]any{
		"effect_id": "repo.read", "requirement_id": "read-input",
		"scope_profile": "repo_read",
	}
	if _, err := validatePermission(valid); err != nil {
		t.Fatalf("valid permission: %v", err)
	}
	valid["scope_profile"] = "repo_write_exact"
	if _, err := validatePermission(valid); err == nil {
		t.Fatal("wrong effect scope profile accepted")
	}
}

func testContentRef(path, selector, digest string, size int64) map[string]any {
	return map[string]any{
		"content_bytes": size, "content_sha256": digest, "media_type": "application/json",
		"path": path, "selector": selector,
	}
}

func canonicalSortObjects(t *testing.T, values []any) {
	t.Helper()
	first, _ := canonicalJSON(values[0])
	second, _ := canonicalJSON(values[1])
	if string(first) > string(second) {
		values[0], values[1] = values[1], values[0]
	}
}
