package approvalcontextstore

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/approvalcontext"
)

func TestWriteLoadExactPrivateContext(t *testing.T) {
	root := t.TempDir()
	context := storeContextFixture("design")
	digest, err := Write(root, context)
	if err != nil {
		t.Fatal(err)
	}
	path, _ := Path(root, "design")
	data, err := os.ReadFile(path)
	if err != nil || len(data) == 0 || data[len(data)-1] == '\n' {
		t.Fatalf("wire = %q, %v", data, err)
	}
	directory, _ := os.Stat(filepath.Dir(path))
	file, _ := os.Stat(path)
	if directory.Mode().Perm() != 0o700 || file.Mode().Perm() != 0o600 {
		t.Fatalf("modes = %#o/%#o", directory.Mode().Perm(), file.Mode().Perm())
	}
	loaded, loadedDigest, err := Load(root, "design")
	if err != nil || loaded != context || loadedDigest != digest {
		t.Fatalf("load = %#v, %q, %v", loaded, loadedDigest, err)
	}
}

func TestLoadRejectsWireIdentityAndPermissionDrift(t *testing.T) {
	root := t.TempDir()
	context := storeContextFixture("design")
	if _, err := Write(root, context); err != nil {
		t.Fatal(err)
	}
	path, _ := Path(root, "design")
	data, _ := os.ReadFile(path)
	if err := os.WriteFile(path, append(data, '\n'), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, _, err := Load(root, "design"); err == nil {
		t.Fatal("context with trailing LF accepted")
	}
	if _, err := Write(root, context); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(path, 0o640); err != nil {
		t.Fatal(err)
	}
	if _, _, err := Load(root, "design"); err == nil {
		t.Fatal("non-private context accepted")
	}
}

func TestPathAndLoadRejectSwapAndAliases(t *testing.T) {
	for _, stage := range []string{"", "../design", "build", "Design"} {
		if _, err := Path(t.TempDir(), stage); err == nil {
			t.Fatalf("unsafe stage %q accepted", stage)
		}
	}
	root := t.TempDir()
	context := storeContextFixture("design")
	data, _ := approvalcontext.CanonicalContextJSON(context)
	directory := filepath.Join(root, ".forge")
	if err := os.Mkdir(directory, 0o700); err != nil {
		t.Fatal(err)
	}
	path, _ := Path(root, "deploy")
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatal(err)
	}
	if _, _, err := Load(root, "deploy"); err == nil {
		t.Fatal("design context accepted from deploy path")
	}
}

func TestWriteRejectsPreplantedAliasLeaves(t *testing.T) {
	for _, test := range []struct {
		name string
		link func(string, string) error
	}{
		{"hardlink", os.Link},
		{"symlink", os.Symlink},
	} {
		t.Run(test.name, func(t *testing.T) {
			root, outside := t.TempDir(), filepath.Join(t.TempDir(), "outside")
			if err := os.Mkdir(filepath.Join(root, ".forge"), 0o700); err != nil {
				t.Fatal(err)
			}
			if err := os.WriteFile(outside, []byte("sentinel"), 0o600); err != nil {
				t.Fatal(err)
			}
			path, _ := Path(root, "design")
			if err := test.link(outside, path); err != nil {
				t.Skipf("%s unavailable: %v", test.name, err)
			}
			if _, err := Write(root, storeContextFixture("design")); err == nil {
				t.Fatalf("preplanted %s accepted", test.name)
			}
			data, _ := os.ReadFile(outside)
			if string(data) != "sentinel" {
				t.Fatalf("outside target changed: %q", data)
			}
		})
	}
}

func storeContextFixture(stage string) approvalcontext.Context {
	digest := strings.Repeat("a", 64)
	return approvalcontext.Context{
		Format: approvalcontext.ContextFormat, AgentOutputReceiptSHA256: digest,
		ArtifactInputsSHA256: digest, ArtifactOutputsSHA256: digest,
		CreatedAtUnixMS: 2, LocalRuntimePolicySHA256: digest,
		PromptContextSHA256: digest, RunID: "run-1", SourceAfterSHA256: digest,
		Stage: stage, Workflow: stage, WorkflowSHA256: digest,
	}
}
