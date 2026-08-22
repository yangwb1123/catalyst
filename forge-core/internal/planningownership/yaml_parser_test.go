package planningownership

import (
	"bytes"
	"fmt"
	"strings"
	"testing"
)

func TestStrictYAMLAcceptsFrozenConstructs(t *testing.T) {
	raw := []byte("root:\n  sequence: [plain, \"quoted # & * ! ' data\", true, false, null, -7, [], {}]\n  mapping: {a: one, \"b\": two}\n  block: >-\n    \"first line\"\n    second line\n\nbare:\n  -\n    value: ok\nplain_value: abc-def\n")
	value, err := decodeStrictYAML(raw, len(raw))
	if err != nil {
		t.Fatal(err)
	}
	root := value.(map[string]any)["root"].(map[string]any)
	if root["block"] != `"first line" second line` || value.(map[string]any)["plain_value"] != "abc-def" {
		t.Fatalf("folded/plain scalar decode drifted: %#v", value)
	}
	flow := root["mapping"].(map[string]any)
	if flow["a"] != "one" || flow["b"] != "two" {
		t.Fatalf("flow mapping decode drifted: %#v", flow)
	}
}

func TestStrictYAMLRejectsForbiddenSyntaxAndCoercion(t *testing.T) {
	for _, cases := range []map[string]string{forbiddenYAMLLexicalCases(), forbiddenYAMLScalarCases()} {
		for name, source := range cases {
			t.Run(name, func(t *testing.T) {
				if _, err := decodeStrictYAML([]byte(source), len(source)); err == nil {
					t.Fatalf("forbidden YAML accepted: %q", source)
				}
			})
		}
	}
}

func forbiddenYAMLLexicalCases() map[string]string {
	return map[string]string{
		"comment":                         "a: value # comment\n",
		"anchor":                          "a: &anchor value\n",
		"alias":                           "a: *anchor\n",
		"merge":                           "a:\n  <<: value\n",
		"tag":                             "a: !str value\n",
		"hash-anywhere":                   "a: value#data\n",
		"anchor-middle":                   "a: value&data\n",
		"alias-middle":                    "a: value*data\n",
		"tag-middle":                      "a: value!data\n",
		"backslash-plain":                 "a: value\\data\n",
		"directive":                       "%YAML 1.2\na: value\n",
		"document-start":                  "---\na: value\n",
		"document-end":                    "a: value\n...\n",
		"sequence-document-start":         "- ---\n",
		"sequence-directive-prefix":       "- %foo\n",
		"single-quote":                    "a: 'value'\n",
		"escape":                          "a: \"bad\\n\"\n",
		"literal-block":                   "a: |\n  value\n",
		"folded-clip":                     "a: >\n  value\n",
		"trailing-flow":                   "a: [one] junk\n",
		"flow-comma":                      "a: [one,]\n",
		"block-blank":                     "a: >-\n  one\n\n  two\nb: ok\n",
		"block-indent":                    "a: >-\n    too-deep\n",
		"folded-comment":                  "a: >-\n  one # data\n",
		"folded-backslash":                "a: >-\n  one\\data\n",
		"folded-quoted-indicator":         "a: >-\n  \"one#data\"\n",
		"folded-unmatched-quote":          "a: >-\n  unmatched\"\n",
		"folded-directive":                "a: >-\n  %YAML 1.2\n",
		"folded-document-marker":          "a: >-\n  ---\n",
		"folded-merge":                    "a: >-\n  <<: data\n",
		"colon-no-space":                  "a:value\n",
		"sequence-colon-no-space":         "a:\n  - a:x\n",
		"sequence-unmatched-square-colon": "a:\n  - a]: b\n",
		"sequence-unmatched-curly-colon":  "a:\n  - a}: b\n",
		"sequence-two-spaces":             "a:\n  -  x\n",
		"sequence-three-spaces":           "a:\n  -   x\n",
		"colon-two-spaces":                "a:  value\n",
		"flow-colon-no-space":             "a: {b:value}\n",
		"flow-colon-two-spaces":           "a: {b:  value}\n",
		"flow-space-before-colon":         "a: {b : value}\n",
		"quoted-flow-space-before-colon":  "a: {\"b\" : value}\n",
		"key-leading-punctuation":         "_a: value\n",
		"quoted-key-leading-punctuation":  "\"_a\": value\n",
	}
}

func forbiddenYAMLScalarCases() map[string]string {
	return map[string]string{
		"timestamp":            "a: 2026-08-13\n",
		"float":                "a: 1.25\n",
		"infinity":             "a: .Inf\n",
		"nan":                  "a: .nan\n",
		"signed-nan":           "a: +nan\n",
		"signed-dot-nan":       "a: -.NaN\n",
		"yaml11-bool":          "a: YES\n",
		"yaml11-single-y":      "a: Y\n",
		"yaml11-y":             "a: y\n",
		"yaml11-n":             "a: N\n",
		"leading-zero":         "a: 01\n",
		"negative-zero":        "a: -0\n",
		"positive-sign":        "a: +1\n",
		"plain-indicator":      "a: @value\n",
		"plain-trailing-colon": "a: value:\n",
		"date-short":           "a: 2026-8-3\n",
		"time-lower-z":         "a: 1:02:03z\n",
		"time-broad-offset":    "a: 1:02+123:45\n",
		"scientific":           "a: 1e3\n",
		"hex":                  "a: 0x10\n",
		"integer-overflow":     "a: 9223372036854775808\n",
	}
}

func TestStrictYAMLRejectsFramingAndDuplicateKeys(t *testing.T) {
	cases := [][]byte{
		{}, []byte("a: b"), []byte("a: b\n\n"), []byte("a: b\r\n"),
		[]byte("a:\tb\n"), []byte(" a: b\n"), []byte("a: b \n"),
		append([]byte{0xef, 0xbb, 0xbf}, []byte("a: b\n")...),
		[]byte("a: one\na: two\n"), []byte("a:\n  x: one\n  \"x\": two\n"),
		[]byte("a: {x: one, \"x\": two}\n"),
	}
	for index, raw := range cases {
		if _, err := decodeStrictYAML(raw, maxCatalogSourceBytes); err == nil {
			t.Fatalf("framing/duplicate case %d accepted", index)
		}
	}
}

func TestStrictYAMLRejectsEveryC0AndDELByte(t *testing.T) {
	for character := 0; character <= 0x7f; character++ {
		if character == '\n' || character >= 0x20 && character < 0x7f {
			continue
		}
		raw := []byte{'a', ':', ' ', byte(character), '\n'}
		if _, err := decodeStrictYAML(raw, maxCatalogSourceBytes); err == nil {
			t.Fatalf("forbidden byte 0x%02x accepted", character)
		}
	}
}

func TestStrictYAMLUsesOneCommonMappingKeyGrammar(t *testing.T) {
	raw := []byte("A9_b-c.d/e: {B2/c: ok}\nquoted: {\"D3/e\": []}\n")
	if _, err := decodeStrictYAML(raw, len(raw)); err != nil {
		t.Fatalf("common block/flow/quoted key grammar rejected: %v", err)
	}
}

func TestStrictYAMLAcceptsInternalCollectionBytesInPlainScalars(t *testing.T) {
	raw := []byte("block:\n  - a[\n  - a]\n  - a{\n  - a}\n  - x[y:z]\n  - a,\n  - a,b\n  - a[,\n  - ...\nflow: [a[, a{, a, b]\n")
	value, err := decodeStrictYAML(raw, len(raw))
	if err != nil {
		t.Fatal(err)
	}
	object := value.(map[string]any)
	wantBlock := []any{"a[", "a]", "a{", "a}", "x[y:z]", "a,", "a,b", "a[,", "..."}
	if fmt.Sprint(object["block"]) != fmt.Sprint(wantBlock) ||
		fmt.Sprint(object["flow"]) != fmt.Sprint([]any{"a[", "a{", "a", "b"}) {
		t.Fatalf("plain scalar collection bytes drifted: %#v", object)
	}
}

func TestStrictYAMLAcceptsEllipsisAsSequenceScalar(t *testing.T) {
	value, err := decodeStrictYAML([]byte("- ...\n"), len("- ...\n"))
	if err != nil {
		t.Fatal(err)
	}
	if sequence := value.([]any); len(sequence) != 1 || sequence[0] != "..." {
		t.Fatalf("ellipsis sequence scalar drifted: %#v", value)
	}
}

func TestStrictYAMLSharedParserCorpus(t *testing.T) {
	accepted := []byte("A9_b-c.d/e: {\"B2/c\": [], C3: {}}\nquoted: \"value#&*!'\"\nnested:\n  -\n    key: value\n")
	if _, err := decodeStrictYAML(accepted, len(accepted)); err != nil {
		t.Fatalf("shared accepted YAML corpus rejected: %v", err)
	}
	for _, indicator := range []byte("#&*!'\\") {
		for _, raw := range [][]byte{
			[]byte("a: value" + string(indicator) + "data\n"),
			[]byte("a: >-\n  value" + string(indicator) + "data\n"),
		} {
			if _, err := decodeStrictYAML(raw, len(raw)); err == nil {
				t.Fatalf("shared forbidden YAML corpus accepted: %q", raw)
			}
		}
	}
}

func TestStrictYAMLCanonicalInt64Boundaries(t *testing.T) {
	raw := []byte("minimum: -9223372036854775808\nmaximum: 9223372036854775807\nzero: 0\n")
	value, err := decodeStrictYAML(raw, len(raw))
	if err != nil {
		t.Fatal(err)
	}
	object := value.(map[string]any)
	if object["minimum"] != int64(-9223372036854775808) ||
		object["maximum"] != int64(9223372036854775807) || object["zero"] != int64(0) {
		t.Fatalf("canonical int64 typing drifted: %#v", object)
	}
}

func TestStrictYAMLCollectionAndScalarBounds(t *testing.T) {
	fieldsAt := buildMappingFields(maxYAMLFields)
	if _, err := decodeStrictYAML(fieldsAt, len(fieldsAt)); err != nil {
		t.Fatalf("mapping at bound rejected: %v", err)
	}
	if raw := buildMappingFields(maxYAMLFields + 1); mustRejectYAML(raw) == nil {
		t.Fatal("mapping N+1 accepted")
	}
	itemsAt := buildSequenceItems(maxYAMLItems)
	if _, err := decodeStrictYAML(itemsAt, len(itemsAt)); err != nil {
		t.Fatalf("sequence at bound rejected: %v", err)
	}
	if raw := buildSequenceItems(maxYAMLItems + 1); mustRejectYAML(raw) == nil {
		t.Fatal("sequence N+1 accepted")
	}
	scalarAt := []byte("a: " + strings.Repeat("x", maxYAMLScalarBytes) + "\n")
	if _, err := decodeStrictYAML(scalarAt, len(scalarAt)); err != nil {
		t.Fatalf("scalar at bound rejected: %v", err)
	}
	scalarOver := []byte("a: " + strings.Repeat("x", maxYAMLScalarBytes+1) + "\n")
	if mustRejectYAML(scalarOver) == nil {
		t.Fatal("scalar N+1 accepted")
	}
}

func TestFoldedScalarTotalBoundAndTokenAccounting(t *testing.T) {
	atBound := []byte("a: >-\n  " + strings.Repeat("x", maxYAMLScalarBytes/2) + "\n  " +
		strings.Repeat("y", maxYAMLScalarBytes-maxYAMLScalarBytes/2-1) + "\n")
	lines, err := prepareYAMLLines(atBound, len(atBound))
	if err != nil {
		t.Fatal(err)
	}
	parser := &yamlParser{lines: lines}
	if _, err := parser.parseNode(0, 1); err != nil || parser.tokens != 3 {
		t.Fatalf("folded bound/token accounting = tokens %d error %v", parser.tokens, err)
	}
	over := append(cloneBytes(atBound[:len(atBound)-1]), 'y', '\n')
	if mustRejectYAML(over) == nil {
		t.Fatal("folded scalar accumulated N+1 bytes accepted")
	}
}

func TestStrictYAMLDepthAndGlobalCounters(t *testing.T) {
	atDepth := buildNestedMapping(maxYAMLDepth - 2)
	if _, err := decodeStrictYAML(atDepth, len(atDepth)); err != nil {
		t.Fatalf("depth boundary rejected: %v", err)
	}
	if mustRejectYAML(buildNestedMapping(maxYAMLDepth-1)) == nil {
		t.Fatal("depth N+1 accepted")
	}
	parser := &yamlParser{tokens: maxYAMLTokens - 1, collections: maxYAMLCollections - 1}
	if err := parser.addToken(); err != nil || parser.addToken() == nil {
		t.Fatal("token boundary did not accept N and reject N+1")
	}
	parser.tokens = 0
	if err := parser.addCollection(1); err != nil || parser.addCollection(1) == nil {
		t.Fatal("collection boundary did not accept N and reject N+1")
	}
}

func TestStrictYAMLSourceByteBoundary(t *testing.T) {
	atBound := append(bytes.Repeat([]byte{'a'}, maxCatalogSourceBytes-1), '\n')
	if _, err := prepareYAMLLines(atBound, maxCatalogSourceBytes); err != nil {
		t.Fatalf("source at byte bound rejected: %v", err)
	}
	over := append(bytes.Repeat([]byte{'a'}, maxCatalogSourceBytes), '\n')
	if _, err := prepareYAMLLines(over, maxCatalogSourceBytes); err == nil {
		t.Fatal("source byte N+1 accepted")
	}
}

func buildMappingFields(count int) []byte {
	var builder strings.Builder
	for index := 0; index < count; index++ {
		fmt.Fprintf(&builder, "k%d: x\n", index)
	}
	return []byte(builder.String())
}

func buildSequenceItems(count int) []byte {
	return []byte(strings.Repeat("- x\n", count))
}

func buildNestedMapping(collections int) []byte {
	var builder strings.Builder
	for depth := 0; depth < collections; depth++ {
		builder.WriteString(strings.Repeat("  ", depth))
		builder.WriteString("a:\n")
	}
	builder.WriteString(strings.Repeat("  ", collections))
	builder.WriteString("value: x\n")
	return []byte(builder.String())
}

func mustRejectYAML(raw []byte) error {
	_, err := decodeStrictYAML(raw, maxCatalogSourceBytes)
	return err
}
