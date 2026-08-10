package evolvelocatorobservationproducer

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"

	"forgeos/forge-core/internal/evolvescan"
	"forgeos/forge-core/internal/gitworktreesource"
)

func testRepository(t *testing.T, files map[string]string) string {
	t.Helper()
	root := t.TempDir()
	for path, content := range files {
		full := filepath.Join(root, filepath.FromSlash(path))
		if err := os.MkdirAll(filepath.Dir(full), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(full, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	runGit(t, root, "init", "-q")
	runGit(t, root, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "add", ".")
	runGit(t, root, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture")
	return root
}

func runGit(t *testing.T, root string, args ...string) {
	t.Helper()
	command := exec.Command("git", append([]string{"-C", root}, args...)...)
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("git %v: %v\n%s", args, err, output)
	}
}

func encodedReport(t *testing.T, report evolvescan.Report) string {
	t.Helper()
	data, err := json.Marshal(report)
	if err != nil {
		t.Fatal(err)
	}
	return "narrative\n" + evolvescan.MarkerPrefix + string(data)
}

func standardReport() evolvescan.Report {
	return evolvescan.Report{
		Version: evolvescan.ContractV1, Depth: evolvescan.DepthStandard,
		Dimensions: []evolvescan.Dimension{
			{
				Name: "security", Status: evolvescan.StatusClear,
				Evidence: []evolvescan.Evidence{{
					Path: "evidence/security.txt", Line: 1, Detail: "security boundary inspected",
				}},
			},
			{
				Name: "code", Status: evolvescan.StatusFinding,
				Evidence: []evolvescan.Evidence{{
					Path: "evidence/code.txt", Line: 1, Detail: "code boundary needs repair",
				}},
			},
		},
		Opportunities: []evolvescan.Opportunity{{
			ID: "code-repair", Dimension: "code", Title: "repair code boundary",
			Evidence: []evolvescan.Evidence{{
				Path: "evidence/code.txt", Line: 1, Detail: "same file supports opportunity",
			}},
			Obvious: true,
		}},
	}
}

func fixedClock() time.Time { return time.UnixMilli(1_786_345_200_123) }

func produceFixture(t *testing.T, root string, report evolvescan.Report) *Production {
	t.Helper()
	production, err := produceWith(
		context.Background(), root, encodedReport(t, report), report.Depth,
		"run-evolve-capture", os.Environ(), fixedClock, realSourceCapture,
	)
	if err != nil {
		t.Fatal(err)
	}
	return production
}

func realSourceCapture(ctx context.Context, root string, environment []string) (gitworktreesource.Snapshot, error) {
	return gitworktreesource.Capture(ctx, root, environment)
}

func details(prefix string, count int) []evolvescan.Evidence {
	result := make([]evolvescan.Evidence, count)
	for index := range result {
		result[index] = evolvescan.Evidence{
			Path: "evidence/all.txt", Line: 1, Detail: fmt.Sprintf("%s evidence %02d", prefix, index),
		}
	}
	return result
}
