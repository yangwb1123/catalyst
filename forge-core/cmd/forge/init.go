// init.go — `forge init`: scaffold a new ForgeOS-governed project by shelling
// to harness/scaffold/forge-init.mjs. This is the user-facing entry point for
// creating a new project with the full governance harness (red-lines, agent
// cards, workflows, acceptance gate, CI). Delegates to the Node.js tool so all
// scaffold logic stays in one place.
package main

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
)

// cmdInit implements `forge init --name <project> [--mode ...] <target-dir>`.
// Flags MUST come before the positional target-dir (Go's flag package convention).
// Shells to harness/scaffold/forge-init.mjs with the same arguments.
func cmdInit(args []string) int {
	fs := flag.NewFlagSet("init", flag.ContinueOnError)
	name := fs.String("name", "", "project name (required)")
	mode := fs.String("mode", "balanced", "engineering mode (explorer|balanced|engineering|cto)")
	lifecycle := fs.String("lifecycle", "mvp", "project maturity (idea|mvp|growth|production)")
	force := fs.Bool("force", false, "clobber an existing .agent directory")
	root := fs.String("root", "", "repo root; used to locate harness/scaffold/forge-init.mjs")
	if err := fs.Parse(args); err != nil {
		return 2
	}
	targetDir := fs.Arg(0)
	if *name == "" {
		fmt.Fprintln(os.Stderr, "forge init: --name is required")
		return 2
	}
	if targetDir == "" {
		fmt.Fprintln(os.Stderr, "forge init: <target-dir> is required")
		return 2
	}
	shim := locateInitShim(*root)
	if shim == "" {
		fmt.Fprintf(os.Stderr, "forge init: forge-init.mjs not found. Use --root to specify the ForgeOS repo root\n")
		return 1
	}
	// Forge-init.mjs expects: <target-dir> --name <project> [--mode ...] [--lifecycle ...] [--force]
	nodeArgs := []string{shim, targetDir, "--name", *name, "--mode", *mode, "--lifecycle", *lifecycle}
	if *force {
		nodeArgs = append(nodeArgs, "--force")
	}
	cmd := exec.Command("node", nodeArgs...)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	if err := cmd.Run(); err != nil {
		fmt.Fprintf(os.Stderr, "forge init: scaffold failed: %v\n", err)
		return 1
	}
	return 0
}

// locateInitShim finds forge-init.mjs by searching --root, binary location,
// and cwd. Returns "" when not found.
func locateInitShim(root string) string {
	if root != "" {
		candidate := filepath.Join(root, "harness", "scaffold", "forge-init.mjs")
		if _, err := os.Stat(candidate); err == nil {
			return candidate
		}
	}
	if exe, err := os.Executable(); err == nil {
		for _, rel := range []string{
			filepath.Join("..", "..", "harness", "scaffold", "forge-init.mjs"),
			filepath.Join("harness", "scaffold", "forge-init.mjs"),
		} {
			candidate := filepath.Join(filepath.Dir(exe), rel)
			if _, err := os.Stat(candidate); err == nil {
				return candidate
			}
		}
	}
	candidate := filepath.Join("harness", "scaffold", "forge-init.mjs")
	if _, err := os.Stat(candidate); err == nil {
		return candidate
	}
	return ""
}
