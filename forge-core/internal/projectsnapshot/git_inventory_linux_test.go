//go:build linux

package projectsnapshot

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestCaptureRejectsDuplicateAndMalformedGitInventories(t *testing.T) {
	oid := strings.Repeat("1", 40)
	tracked := "100644 " + oid + " 0\\tpublic.txt"
	tests := []struct {
		name, kind, payload string
	}{
		{"tracked-duplicate", "tracked", tracked + "\\000" + tracked + "\\000"},
		{"tracked-malformed", "tracked", tracked},
		{"tracked-unmerged-stage", "tracked", "100644 " + oid + " 1\\tpublic.txt\\000"},
		{"untracked-duplicate", "untracked", "new.txt\\000new.txt\\000"},
		{"untracked-malformed", "untracked", "new.txt"},
		{"ignored-duplicate", "ignored", "ignored.txt\\000ignored.txt\\000"},
		{"ignored-malformed", "ignored", "ignored.txt"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			root, _ := snapshotFixture(t)
			environment := writeInventoryOverrideGit(t, test.kind, test.payload)
			production, err := Capture(context.Background(), root, environment, "project-1", "run-1")
			if err == nil || production != nil {
				t.Fatalf("invalid Git inventory returned production=%v, err=%v", production != nil, err)
			}
		})
	}
}

func writeInventoryOverrideGit(t *testing.T, kind, payload string) []string {
	t.Helper()
	realGit, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	directory := t.TempDir()
	logic := inventoryOverrideLogic(kind, payload)
	script := "#!/bin/sh\n" + logic + fmt.Sprintf("exec %q \"$@\"\n", realGit)
	if err := os.WriteFile(filepath.Join(directory, "git"), []byte(script), 0o755); err != nil {
		t.Fatal(err)
	}
	return []string{"PATH=" + directory}
}

func inventoryOverrideLogic(kind, payload string) string {
	return fmt.Sprintf(`stage=false; others=false; ignored=false
for arg do
  [ "$arg" = "--stage" ] && stage=true
  [ "$arg" = "--others" ] && others=true
  [ "$arg" = "--ignored" ] && ignored=true
done
override=false
[ %q = tracked ] && [ "$stage" = true ] && override=true
[ %q = untracked ] && [ "$others" = true ] && [ "$ignored" = false ] && override=true
[ %q = ignored ] && [ "$ignored" = true ] && override=true
[ "$override" = true ] && { printf %q; exit 0; }
`, kind, kind, kind, payload)
}
