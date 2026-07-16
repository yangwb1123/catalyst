// approve.go — split out of gates.go (which was pressed against the 500-line
// volume gate) to keep both files comfortably under it. This file owns the
// `forge approve list` CLI subcommand: a read-only scan of the .forge/*.approved
// human-gate markers gates.go's humanApproved/approvalPath helpers write and read.
package main

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/persist"
)

// ── forge approve list ──────────────────────────────────────────────────────

// cmdApprove implements `forge approve list [--root DIR]` — lists all pending
// human-gate approvals by scanning .forge/*.approved markers.
// Future: `forge approve <stage> --yes` to approve, `forge reject <stage> --reason "..."`.
func cmdApprove(args []string) int {
	fs := flag.NewFlagSet("approve", flag.ContinueOnError)
	var root string
	fs.StringVar(&root, "root", "", "repo root (default $FORGE_REPO_ROOT or .)")
	if err := fs.Parse(args); err != nil {
		return 2
	}
	root = gate.RepoRoot(root)
	rest := fs.Args()

	if len(rest) > 0 && rest[0] == "list" {
		return cmdApproveList(root)
	}
	// --help or no args: show usage
	fmt.Fprint(os.Stderr, `usage: forge approve list [--root DIR]

  List pending human-gate approvals by scanning .forge/*.approved markers.
`)
	return 2
}

// cmdApproveList lists pending human approvals.
func cmdApproveList(root string) int {
	dotForge := forgeDir(root)
	_, err := os.Stat(dotForge)
	if os.IsNotExist(err) {
		fmt.Println("forge approve: no pending approvals (no .forge directory)")
		return 0
	}
	markers, err := filepath.Glob(filepath.Join(dotForge, "*.approved"))
	if err != nil || len(markers) == 0 {
		fmt.Println("forge approve: no pending approvals")
		return 0
	}
	fmt.Println("Pending approvals:")
	for _, m := range markers {
		stage := strings.TrimSuffix(filepath.Base(m), ".approved")
		// Check if there's a corresponding checkpoint for context
		cp, found, _ := persist.Load(filepath.Join(dotForge, "checkpoint.json"))
		cpInfo := ""
		if found {
			cpInfo = fmt.Sprintf(" (checkpoint: iteration=%d, roadmap=%.0f%%)", cp.Iteration, cp.RoadmapCompletion*100)
		}
		fmt.Printf("  %s%s\n", stage, cpInfo)
	}
	fmt.Println("\nTo approve: forge run <stage> --approved")
	return 0
}
