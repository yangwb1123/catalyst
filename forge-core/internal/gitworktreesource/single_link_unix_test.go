//go:build unix

package gitworktreesource

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestSingleLinkReadRejectsHardLinkCreatedAfterContentRead(t *testing.T) {
	root, environment := sourceFixture(t)
	snapshot, err := Capture(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	leaf := filepath.Join(root, "tracked.txt")
	outside := filepath.Join(t.TempDir(), "outside-link")
	mutated := false
	_, err = readRegularFilesWithPolicy(
		context.Background(), snapshot, []string{"tracked.txt"}, testReadLimits(),
		func(stage, _ string) {
			if stage == regularReadAfterContent && !mutated {
				mutated = true
				if linkErr := os.Link(leaf, outside); linkErr != nil {
					t.Fatal(linkErr)
				}
			}
		}, true,
	)
	if !mutated || err == nil || !strings.Contains(err.Error(), "single-link") {
		t.Fatalf("hard-link race error = %v, mutated=%v", err, mutated)
	}
}
