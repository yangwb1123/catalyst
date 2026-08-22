package adrv2

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestValidateDocumentAcceptsExactProposedV2(t *testing.T) {
	document, err := ValidateDocument("ADR-0008-test-boundary.md", validTestDocument(t))
	if err != nil {
		t.Fatalf("valid ADR v2: %v", err)
	}
	if document.Frontmatter.ADRID != "ADR-0008" ||
		!strings.HasPrefix(string(document.Body), "# ADR-0008: Test Boundary\n") {
		t.Fatalf("unexpected detached document: %+v", document)
	}
}

func TestGoldenDocumentHasExactGoParity(t *testing.T) {
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "ADR-9001-proposed-boundary.md")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(data)
	if actual := hex.EncodeToString(digest[:]); actual != "b37dba8cc6d2750bb0ed73c7ee5b3ae61ad25551ec258584ed14618f1cb5c194" {
		t.Fatalf("ADR v2 golden SHA-256 = %s", actual)
	}
	if _, err := ValidateDocument(filepath.Base(path), data); err != nil {
		t.Fatalf("ADR v2 golden: %v", err)
	}
}

func TestValidateDocumentRejectsFramingAndCanonicalMutations(t *testing.T) {
	valid := validTestDocument(t)
	mutations := []struct {
		name   string
		mutate func([]byte) []byte
		want   string
	}{
		{"bom", func(value []byte) []byte { return append([]byte{0xef, 0xbb, 0xbf}, value...) }, "begin"},
		{"crlf", func(value []byte) []byte { return bytes.ReplaceAll(value, []byte("\n"), []byte("\r\n")) }, "delimiter"},
		{"multiline frontmatter", func(value []byte) []byte {
			return bytes.Replace(value, []byte(`,"adr_id"`), []byte(",\n\"adr_id\""), 1)
		}, "one JSON line"},
		{"reordered keys", func(value []byte) []byte {
			return bytes.Replace(value, []byte(`{"acceptance_id":null,"accepted_at_unix_ms":null`), []byte(`{"accepted_at_unix_ms":null,"acceptance_id":null`), 1)
		}, "canonical"},
		{"unknown key", func(value []byte) []byte {
			return bytes.Replace(value, []byte(`{"acceptance_id"`), []byte(`{"alien":null,"acceptance_id"`), 1)
		}, "canonical"},
		{"duplicate key", func(value []byte) []byte {
			return bytes.Replace(value, []byte(`{"acceptance_id":null`), []byte(`{"acceptance_id":null,"acceptance_id":null`), 1)
		}, "duplicate"},
		{"JSON C1 control", func(value []byte) []byte {
			return bytes.Replace(value, []byte("Legacy ADRs"), []byte("Legacy\u0085ADRs"), 1)
		}, "forbidden Unicode"},
		{"body tab", func(value []byte) []byte {
			return bytes.Replace(value, []byte("The runtime needs"), []byte("The runtime\tneeds"), 1)
		}, "forbidden Unicode"},
		{"body C1 control", func(value []byte) []byte {
			return bytes.Replace(value, []byte("The runtime needs"), []byte("The runtime\u0085needs"), 1)
		}, "forbidden Unicode"},
		{"final blank line", func(value []byte) []byte { return append(value, '\n') }, "exactly one LF"},
		{"trailing space", func(value []byte) []byte {
			return bytes.Replace(value, []byte("## Context\n"), []byte("## Context \n"), 1)
		}, "trailing whitespace"},
	}
	for _, test := range mutations {
		t.Run(test.name, func(t *testing.T) {
			_, err := ValidateDocument("ADR-0008-test-boundary.md", test.mutate(append([]byte(nil), valid...)))
			if err == nil || !strings.Contains(err.Error(), test.want) {
				t.Fatalf("mutation error = %v, want %q", err, test.want)
			}
		})
	}
}

func TestValidateDocumentRejectsBoundIdentityBodyAndDigestMutations(t *testing.T) {
	valid := validTestDocument(t)
	tests := []struct{ name, filename, old, replacement, want string }{
		{"filename", "ADR-0009-test-boundary.md", "unused", "unused", "document_name"},
		{"body", "ADR-0008-test-boundary.md", "Malformed records fail closed.", "Malformed records fail open.", "body_sha256 mismatch"},
		{"heading", "ADR-0008-test-boundary.md", "## Validation", "## Verification", "cannot find section"},
		{"self digest", "ADR-0008-test-boundary.md", `"self_sha256":"`, `"self_sha256":"f`, "lowercase bare SHA-256"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			mutated := valid
			if test.old != "unused" {
				mutated = bytes.Replace(valid, []byte(test.old), []byte(test.replacement), 1)
			}
			_, err := ValidateDocument(test.filename, mutated)
			if err == nil || !strings.Contains(err.Error(), test.want) {
				t.Fatalf("mutation error = %v, want %q", err, test.want)
			}
		})
	}
}

func TestValidateDocumentRejectsCommonMarkLevelTwoSections(t *testing.T) {
	document := validTestDocument(t)
	_, body, err := splitDocument(document)
	if err != nil {
		t.Fatal(err)
	}
	mutations := [][]byte{
		bytes.Replace(body, []byte("\n\n## Limitations"),
			[]byte("\n\n ## Extra\nNo.\n\n## Limitations"), 1),
		bytes.Replace(body, []byte("\n\n## Limitations"),
			[]byte("\n\nExtra\n-----\nNo.\n\n## Limitations"), 1),
	}
	for _, mutated := range mutations {
		root := validTestFrontmatter(mutated)
		_, err := ValidateDocument("ADR-0008-test-boundary.md", sealTestDocument(t, root, mutated))
		if err == nil || !strings.Contains(err.Error(), "extra level-two") {
			t.Fatalf("CommonMark level-two mutation error = %v", err)
		}
	}
}

func TestValidateDocumentRejectsSemanticMutations(t *testing.T) {
	bodyDocument := validTestDocument(t)
	_, body, err := splitDocument(bodyDocument)
	if err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name, want string
		mutate     func(map[string]any)
	}{
		{"blank title", "title must contain", func(root map[string]any) { root["title"] = " \u2003 " }},
		{"missing owner", "owner_refs must be nonempty", func(root map[string]any) { root["owner_refs"] = []any{} }},
		{"expired proposal", "expires_at_unix_ms", func(root map[string]any) { root["expires_at_unix_ms"] = int64(1) }},
		{"control implementation", "implementation_refs", func(root map[string]any) { root["implementation_refs"] = []any{".git/config"} }},
		{"empty evidence declaration", "must be nonempty", func(root map[string]any) {
			root["validation_plan"].([]any)[0].(map[string]any)["evidence_required"] = []any{}
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			root := validTestFrontmatter(body)
			test.mutate(root)
			_, err := ValidateDocument("ADR-0008-test-boundary.md", sealTestDocument(t, root, body))
			if err == nil || !strings.Contains(err.Error(), test.want) {
				t.Fatalf("semantic mutation error = %v, want %q", err, test.want)
			}
		})
	}
}

func TestValidateDocumentReturnsDetachedBody(t *testing.T) {
	input := validTestDocument(t)
	document, err := ValidateDocument("ADR-0008-test-boundary.md", input)
	if err != nil {
		t.Fatal(err)
	}
	before := append([]byte(nil), document.Body...)
	for index := range input {
		input[index] = 'x'
	}
	if !bytes.Equal(document.Body, before) {
		t.Fatal("validated body aliases caller bytes")
	}
}

func TestValidateDocumentRejectsSameLengthSelfDigestMutation(t *testing.T) {
	data := validTestDocument(t)
	marker := []byte(`"self_sha256":"`)
	index := bytes.Index(data, marker)
	if index < 0 {
		t.Fatal("test document has no self_sha256")
	}
	index += len(marker)
	if data[index] == '0' {
		data[index] = '1'
	} else {
		data[index] = '0'
	}
	_, err := ValidateDocument("ADR-0008-test-boundary.md", data)
	if err == nil || !strings.Contains(err.Error(), "self_sha256 mismatch") {
		t.Fatalf("same-length self digest mutation error = %v", err)
	}
}

func TestValidateDocumentRejectsClosedDocumentBound(t *testing.T) {
	_, err := ValidateDocument("ADR-0008-test-boundary.md", make([]byte, MaxDocumentBytes+1))
	if err == nil || !strings.Contains(err.Error(), "1..262144 bytes") {
		t.Fatalf("oversized ADR error = %v", err)
	}
}

func TestValidateDocumentBoundsFilenameBytes(t *testing.T) {
	bodyDocument := validTestDocument(t)
	_, body, err := splitDocument(bodyDocument)
	if err != nil {
		t.Fatal(err)
	}
	for _, length := range []int{255, 256} {
		t.Run(fmt.Sprintf("bytes-%d", length), func(t *testing.T) {
			name := "ADR-0008-" + strings.Repeat("a", length-len("ADR-0008-")-len(".md")) + ".md"
			root := validTestFrontmatter(body)
			root["document_name"] = name
			data := sealTestDocument(t, root, body)
			_, err := ValidateDocument(name, data)
			if length == 255 && err != nil {
				t.Fatalf("255-byte filename: %v", err)
			}
			if length == 256 && (err == nil || !strings.Contains(err.Error(), "document_name")) {
				t.Fatalf("256-byte filename error = %v", err)
			}
		})
	}
}
