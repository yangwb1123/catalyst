//go:build unix

package projectsnapshot

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
)

func TestCaptureRejectsUnsafeGitVersionBytes(t *testing.T) {
	root, _ := snapshotFixture(t)
	realGit, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	tests := []string{
		`git version 2.43.0\377`,
		`git version 2.43\033.0`,
		`git version 2.43\342\200\256.0`,
		`git version 2.43\357\273\277.0`,
		`\tgit version 2.43.0\n`,
		`git version 2.43.0\n\n`,
		`git version 2.43.0 \n`,
		`git version 2.43.0\302\240\n`,
		`git version 2.43.0`,
	}
	for index, payload := range tests {
		t.Run(fmt.Sprintf("unsafe-%d", index), func(t *testing.T) {
			directory := t.TempDir()
			writeFakeGit(t, directory, realGit, payload)
			production, captureErr := Capture(context.Background(), root,
				[]string{"PATH=" + directory}, "project-1", "run-1")
			if captureErr == nil || production != nil {
				t.Fatalf("unsafe Git version capture = %#v, %v", production, captureErr)
			}
		})
	}
}

func TestCaptureRejectsNormalizedGitFactLines(t *testing.T) {
	root, _ := snapshotFixture(t)
	realGit, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name, command, output string
	}{
		{"object-format-leading-tab", "--show-object-format", `\tsha1\n`},
		{"head-extra-newline", "--verify", `1111111111111111111111111111111111111111\n\n`},
		{"toplevel-trailing-space", "--show-toplevel", root + ` \n`},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			directory := t.TempDir()
			writeGitOverride(t, directory, realGit, test.command, test.output)
			production, captureErr := Capture(context.Background(), root,
				[]string{"PATH=" + directory}, "project-1", "run-1")
			if captureErr == nil || production != nil {
				t.Fatalf("normalized Git fact capture = %#v, %v", production, captureErr)
			}
		})
	}
}

func writeFakeGit(t *testing.T, directory, realGit, payload string) {
	t.Helper()
	script := "#!/bin/sh\nlast=\nfor arg do last=$arg; done\n" +
		"if [ \"$last\" = version ]; then printf '" + payload + "'; exit 0; fi\n" +
		"exec \"" + realGit + "\" \"$@\"\n"
	if err := os.WriteFile(filepath.Join(directory, "git"), []byte(script), 0o755); err != nil {
		t.Fatal(err)
	}
}

func writeGitOverride(t *testing.T, directory, realGit, command, output string) {
	t.Helper()
	script := "#!/bin/sh\nfor arg do\n" +
		"  if [ \"$arg\" = \"" + command + "\" ]; then printf '" + output + "'; exit 0; fi\n" +
		"done\nexec \"" + realGit + "\" \"$@\"\n"
	if err := os.WriteFile(filepath.Join(directory, "git"), []byte(script), 0o755); err != nil {
		t.Fatal(err)
	}
}
