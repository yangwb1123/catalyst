package planningownership

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestPureProjectorProductionFilesHaveNoAmbientIOImports(t *testing.T) {
	entries, err := os.ReadDir(".")
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range entries {
		name := entry.Name()
		if entry.IsDir() || !strings.HasSuffix(name, ".go") || strings.HasSuffix(name, "_test.go") ||
			name == "command.go" || name == "command_regular_file.go" || strings.HasPrefix(name, "command_open_") {
			continue
		}
		raw, err := os.ReadFile(filepath.Clean(name))
		if err != nil {
			t.Fatal(err)
		}
		for _, forbidden := range []string{`"os"`, `"path/filepath"`, `"net`, `"os/exec"`, `"time"`} {
			if strings.Contains(string(raw), forbidden) {
				t.Fatalf("pure projector file %s imports ambient facility %s", name, forbidden)
			}
		}
	}
}
