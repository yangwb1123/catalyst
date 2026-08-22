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

func TestCaptureRejectsHEADAndObjectFormatDrift(t *testing.T) {
	tests := []struct {
		name  string
		logic func(string) string
	}{
		{"head-between-passes", headDriftLogic},
		{"object-format-between-passes", objectFormatDriftLogic},
		{"format-head-within-pass-mismatch", objectFormatMismatchLogic},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			root, _ := snapshotFixture(t)
			environment := writeFactDriftingGit(t, test.logic)
			production, err := Capture(context.Background(), root, environment, "project-1", "run-1")
			if err == nil || production != nil {
				t.Fatalf("drifting Git facts returned production=%v, err=%v", production != nil, err)
			}
		})
	}
}

func writeFactDriftingGit(t *testing.T, logic func(string) string) []string {
	t.Helper()
	realGit, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	directory := t.TempDir()
	state := filepath.Join(directory, "fact-state")
	script := "#!/bin/sh\n" + logic(state) + fmt.Sprintf("exec %q \"$@\"\n", realGit)
	if err := os.WriteFile(filepath.Join(directory, "git"), []byte(script), 0o755); err != nil {
		t.Fatal(err)
	}
	return []string{"PATH=" + directory}
}

func headDriftLogic(state string) string {
	return fmt.Sprintf(`head=false
for arg do [ "$arg" = "HEAD" ] && head=true; done
if [ "$head" = true ]; then
  n=0
  [ -r %q ] && IFS= read -r n < %q
  n=$((n + 1)); printf '%%s\n' "$n" > %q
  if [ "$n" -eq 1 ]; then printf '%%s\n' %q; else printf '%%s\n' %q; fi
  exit 0
fi
`, state, state, state, strings.Repeat("1", 40), strings.Repeat("2", 40))
}

func objectFormatDriftLogic(state string) string {
	return fmt.Sprintf(`format=false; head=false
for arg do
  [ "$arg" = "--show-object-format" ] && format=true
  [ "$arg" = "HEAD" ] && head=true
done
if [ "$format" = true ]; then
  n=0
  [ -r %q ] && IFS= read -r n < %q
  n=$((n + 1)); printf '%%s\n' "$n" > %q
  if [ "$n" -eq 1 ]; then printf 'sha1\n'; else printf 'sha256\n'; fi
  exit 0
fi
if [ "$head" = true ]; then
  n=0; [ -r %q ] && IFS= read -r n < %q
  if [ "$n" -eq 1 ]; then printf '%%s\n' %q; else printf '%%s\n' %q; fi
  exit 0
fi
`, state, state, state, state, state, strings.Repeat("1", 40), strings.Repeat("2", 64))
}

func objectFormatMismatchLogic(_ string) string {
	return fmt.Sprintf(`format=false; head=false
for arg do
  [ "$arg" = "--show-object-format" ] && format=true
  [ "$arg" = "HEAD" ] && head=true
done
[ "$format" = true ] && { printf 'sha1\n'; exit 0; }
[ "$head" = true ] && { printf '%%s\n' %q; exit 0; }
`, strings.Repeat("3", 64))
}
