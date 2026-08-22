package authenticatedadrlifecyclecontract

import (
	"os"
	"path/filepath"
	"testing"
)

func repositoryRoot(t *testing.T) string {
	t.Helper()
	root, err := filepath.Abs(filepath.Join("..", "..", ".."))
	if err != nil {
		t.Fatal(err)
	}
	return root
}

func goldenPhysical(t *testing.T) []byte {
	t.Helper()
	path := filepath.Join(repositoryRoot(t), "docs", "contracts", "fixtures",
		"authenticated-architecture-decision-lifecycle-v1.json")
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func goldenInstance(t *testing.T) []byte {
	t.Helper()
	raw := goldenPhysical(t)
	if len(raw) == 0 || raw[len(raw)-1] != '\n' {
		t.Fatal("golden lacks its one physical LF")
	}
	return append([]byte(nil), raw[:len(raw)-1]...)
}

func goldenNode(t *testing.T) map[string]any {
	t.Helper()
	value, err := parseStrictJSON(goldenInstance(t), maxGoldenBytes)
	if err != nil {
		t.Fatal(err)
	}
	return value.(map[string]any)
}
