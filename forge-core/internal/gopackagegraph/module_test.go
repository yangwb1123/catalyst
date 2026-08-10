package gopackagegraph

import (
	"context"
	"strings"
	"testing"
)

func TestParseModuleDirectiveHonorsLexicalBoundaries(t *testing.T) {
	tests := []struct {
		name    string
		content string
	}{
		{name: "simple", content: "module example.com/m\n"},
		{
			name: "after directive block",
			content: "require ( // )\n example.com/dependency v1.0.0\n) // (\n" +
				"module example.com/m\n",
		},
		{
			name:    "line comment contains block markers",
			content: "// /* module example.com/false */\nmodule example.com/m // */\n",
		},
		{
			name: "quoted tokens contain syntax",
			content: "replace example.com/old => `/* ( ) */`\n" +
				"replace example.com/other => \"escaped \\\" /* )\"\nmodule example.com/m\n",
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			modulePath, err := parseModuleDirective(test.content)
			if err != nil || modulePath != "example.com/m" {
				t.Fatalf("module path = %q, error = %v", modulePath, err)
			}
		})
	}
}

func TestParseModuleDirectiveRejectsFalseOrMalformedTopLevel(t *testing.T) {
	tests := []struct {
		name, content, want string
	}{
		{name: "block comment", content: "/*\nmodule example.com/m\n*/", want: "block comments"},
		{
			name: "block comment after module", content: "module example.com/m\n/* invalid */\n",
			want: "block comments",
		},
		{name: "block closer", content: "*/\nmodule example.com/m\n", want: "block comments"},
		{
			name: "module inside block", content: "require (\nmodule example.com/m\n)\n",
			want: "module directive is absent",
		},
		{name: "negative depth", content: ")\nmodule example.com/m\n", want: "closing parenthesis"},
		{name: "nested block", content: "require (\nexclude (\n", want: "nested directive block"},
		{name: "unclosed block", content: "require (\nmodule example.com/m\n", want: "unterminated directive block"},
		{name: "unclosed quote", content: "require \"example.com/x\nmodule example.com/m\n", want: "unterminated quoted token"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			modulePath, err := parseModuleDirective(test.content)
			if err == nil || modulePath != "" || !strings.Contains(err.Error(), test.want) {
				t.Fatalf("module path = %q, error = %v, want %q", modulePath, err, test.want)
			}
		})
	}
}

func TestAnalyzeRejectsPseudoModuleDirectives(t *testing.T) {
	for name, goMod := range map[string]string{
		"block comment":   "/*\nmodule example.com/m\n*/\n",
		"directive block": "require (\nmodule example.com/m\n)\n",
	} {
		t.Run(name, func(t *testing.T) {
			contents := map[string][]byte{"go.mod": []byte(goMod)}
			plan, err := Prepare(".", fixtureEntries(contents))
			if err != nil {
				t.Fatal(err)
			}
			analysis, err := Analyze(
				context.Background(), plan,
				fixtureRegular("go.mod", contents["go.mod"]), []RegularFile{},
			)
			if err == nil || analysis != nil {
				t.Fatalf("analysis = %#v, error = %v", analysis, err)
			}
		})
	}
}
