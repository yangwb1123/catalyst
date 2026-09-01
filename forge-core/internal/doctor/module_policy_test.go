package doctor

import (
	"os"
	"path/filepath"
	"testing"
)

func TestModulePolicyConfinesExternalImports(t *testing.T) {
	for _, test := range []struct {
		name, relative, source string
		wantErr                bool
	}{
		{"sole blank driver", "internal/controlstore/open_linux.go", "package controlstore\nimport _ \"modernc.org/sqlite\"\n", false},
		{"driver in another file", "internal/example/example.go", "package example\nimport _ \"modernc.org/sqlite\"\n", true},
		{"nonblank driver", "internal/controlstore/open_linux.go", "package controlstore\nimport \"modernc.org/sqlite\"\n", true},
		{"other external module", "internal/example/example.go", "package example\nimport _ \"github.com/example/module\"\n", true},
		{"cgo", "internal/example/example.go", "package example\nimport \"C\"\n", true},
	} {
		t.Run(test.name, func(t *testing.T) {
			root := t.TempDir()
			path := filepath.Join(root, filepath.FromSlash(test.relative))
			if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
				t.Fatal(err)
			}
			if err := os.WriteFile(path, []byte(test.source), 0o600); err != nil {
				t.Fatal(err)
			}
			err := checkGoFileImports(root, path)
			if (err != nil) != test.wantErr {
				t.Fatalf("checkGoFileImports() error = %v, wantErr %v", err, test.wantErr)
			}
		})
	}
}
