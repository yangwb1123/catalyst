// detect.go — `forge detect`: project-type detection and workflow suggestion.
//
// Scans the repo root for language/test/CI indicators and maps them to the most
// appropriate ForgeOS workflow + flags. This is the v1 of "adaptive/dynamic loop
// assembly": instead of always requiring the user to pick a workflow manually, detect
// reads observable project signals and outputs a concrete suggested command.
//
// HONEST scope (v1):
//   - Detection is purely STRUCTURAL (file presence) — no semantic analysis of code.
//   - Suggestions are advisory: the user runs the suggested command, not this tool.
//   - Unknown signals → honest "unknown"; never fabricates a confident verdict.
//   - project.yml lifecycle/mode is read if present; missing fields degrade gracefully.
//
// The output format is:
//   forge detect: project analysis
//     language:  <lang>
//     lifecycle: <lifecycle>
//     has-tests: <yes|no>
//     has-ci:    <yes|no>
//     workflow:  <name>  — <reason>
//     command:   forge evolve .agent/workflows/<name>.yml --mode <mode> --lifecycle <lc>
package main

import (
	"bufio"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"forgeos/forge-core/internal/gate"
)

// projectProfile holds the structural facts detected about a project.
type projectProfile struct {
	Language   string   // go | node | python | rust | unknown
	HasTests   bool
	HasCI      bool
	Lifecycle  string   // mvp | production — from project.yml or inferred
	Mode       string   // balanced | engineering | explorer | cto — from project.yml or default
	Indicators []string // human-readable detection hits, for transparency
}

// workflowSuggestion is the output of suggestWorkflow.
type workflowSuggestion struct {
	Workflow string // e.g. "evolve"
	Mode     string // e.g. "balanced"
	Lifecycle string // e.g. "mvp"
	Reason   string // one-sentence why
}

// cmdDetect implements `forge detect [--root DIR]`.
func cmdDetect(args []string) int {
	fs := flag.NewFlagSet("detect", flag.ContinueOnError)
	var root string
	fs.StringVar(&root, "root", "", "repo root (default $FORGE_REPO_ROOT or .)")
	if err := fs.Parse(args); err != nil {
		return 2
	}
	root = gate.RepoRoot(root)
	p := detectProject(root)
	s := suggestWorkflow(p)

	fmt.Printf("forge detect: project analysis\n")
	fmt.Printf("  language:  %s\n", p.Language)
	fmt.Printf("  lifecycle: %s\n", p.Lifecycle)
	fmt.Printf("  has-tests: %s\n", boolStr(p.HasTests))
	fmt.Printf("  has-ci:    %s\n", boolStr(p.HasCI))
	fmt.Printf("  indicators: %s\n", strings.Join(p.Indicators, "; "))
	fmt.Printf("  workflow:  %s  — %s\n", s.Workflow, s.Reason)
	fmt.Printf("  command:   forge evolve .agent/workflows/%s.yml --mode %s --lifecycle %s\n",
		s.Workflow, s.Mode, s.Lifecycle)
	return 0
}

// autoSelectWorkflow runs detection and wires the chosen workflow + flags into o.
// Called by cmdEvolve when the user passes `forge evolve auto`. Returns the detected
// workflow name; mode/lifecycle are written into *o only when not explicitly set via fs.
func autoSelectWorkflow(root string, fs *flag.FlagSet, o *runOpts) string {
	p := detectProject(root)
	s := suggestWorkflow(p)
	fmt.Printf("forge evolve: auto-detected workflow=%q mode=%s lifecycle=%s — %s\n",
		s.Workflow, s.Mode, s.Lifecycle, s.Reason)
	var modeSet, lifecycleSet bool
	fs.Visit(func(f *flag.Flag) {
		if f.Name == "mode" {
			modeSet = true
		}
		if f.Name == "lifecycle" {
			lifecycleSet = true
		}
	})
	if !modeSet {
		o.mode = s.Mode
	}
	if !lifecycleSet {
		o.lifecycle = s.Lifecycle
	}
	return s.Workflow
}

func boolStr(b bool) string {
	if b {
		return "yes"
	}
	return "no"
}

// detectProject scans root for structural project signals and returns a profile.
func detectProject(root string) projectProfile {
	p := projectProfile{Lifecycle: "mvp", Mode: "balanced"}
	p.Language, p.Indicators = detectLanguage(root)
	p.HasTests, p.Indicators = detectTests(root, p.Language, p.Indicators)
	p.HasCI, p.Indicators = detectCI(root, p.Indicators)
	p.Lifecycle, p.Mode, p.Indicators = applyProjectYML(root, p.Lifecycle, p.Mode, p.Indicators)
	if len(p.Indicators) == 0 {
		p.Indicators = []string{"no structural indicators found"}
	}
	return p
}

// detectLanguage returns the primary language and any indicator strings.
func detectLanguage(root string) (lang string, indicators []string) {
	type manifest struct{ file, lang string }
	manifests := []manifest{
		{"go.mod", "go"},
		{"package.json", "node"},
		{"pyproject.toml", "python"},
		{"requirements.txt", "python"},
		{"Cargo.toml", "rust"},
	}
	for _, m := range manifests {
		if fileExists(filepath.Join(root, m.file)) {
			return m.lang, []string{m.file + " found"}
		}
	}
	return "unknown", nil
}

// detectTests checks whether the project has test files for the given language.
func detectTests(root, lang string, ind []string) (hasTests bool, indicators []string) {
	globs := map[string][]string{
		"go":     {"*_test.go"},
		"node":   {"*.test.ts", "*.test.js", "*.spec.ts", "*.spec.js"},
		"python": {"test_*.py", "*_test.py"},
	}
	for _, g := range globs[lang] {
		if dirHasGlob(root, g) {
			return true, append(ind, g+" files found")
		}
	}
	return false, ind
}

// detectCI checks for common CI configuration files.
func detectCI(root string, ind []string) (hasCI bool, indicators []string) {
	ciPaths := []string{
		filepath.Join(root, ".github", "workflows"),
		filepath.Join(root, ".gitlab-ci.yml"),
		filepath.Join(root, "Jenkinsfile"),
	}
	for _, p := range ciPaths {
		if fileExists(p) {
			return true, append(ind, "CI config found")
		}
	}
	return false, ind
}

// applyProjectYML reads lifecycle and mode from .agent/project.yml if present.
func applyProjectYML(root, lifecycle, mode string, ind []string) (string, string, []string) {
	lc, m, ok := readProjectYML(filepath.Join(root, ".agent", "project.yml"))
	if !ok {
		return lifecycle, mode, ind
	}
	if lc != "" {
		lifecycle = lc
		ind = append(ind, fmt.Sprintf("project.yml lifecycle=%s", lc))
	}
	if m != "" {
		mode = m
		ind = append(ind, fmt.Sprintf("project.yml mode=%s", m))
	}
	return lifecycle, mode, ind
}

// suggestWorkflow maps a projectProfile to a workflow + flags recommendation.
func suggestWorkflow(p projectProfile) workflowSuggestion {
	mode := p.Mode
	lifecycle := p.Lifecycle

	// Unknown language with no tests → likely greenfield; start with discovery.
	if p.Language == "unknown" && !p.HasTests {
		return workflowSuggestion{
			Workflow:  "discover",
			Mode:      mode,
			Lifecycle: lifecycle,
			Reason:    "no language manifest + no tests detected → start with requirement discovery",
		}
	}

	// Any project with code: the evolve loop is the standard recommendation.
	// The mode/lifecycle from project.yml (or defaults) shape depth and agent selection.
	reason := "iterative improvement loop for existing project"
	if !p.HasTests {
		reason = "no tests detected — evolve loop will surface and close coverage gaps"
	} else if !p.HasCI {
		reason = "tests present but no CI — evolve loop adds harness gating"
	}
	return workflowSuggestion{
		Workflow:  "evolve",
		Mode:      mode,
		Lifecycle: lifecycle,
		Reason:    reason,
	}
}

// fileExists reports whether the path exists (file or directory).
func fileExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

// dirHasGlob reports whether any file matching pattern exists anywhere under root.
func dirHasGlob(root, pattern string) bool {
	found := false
	_ = filepath.WalkDir(root, func(path string, d os.DirEntry, err error) error {
		if err != nil || found {
			return nil
		}
		if d.IsDir() && (d.Name() == ".git" || d.Name() == "vendor" || d.Name() == "node_modules") {
			return filepath.SkipDir
		}
		if ok, _ := filepath.Match(pattern, d.Name()); ok {
			found = true
		}
		return nil
	})
	return found
}

// readProjectYML reads lifecycle and mode from a project.yml file.
// Returns ("", "", false) when the file is missing or unreadable.
func readProjectYML(path string) (lifecycle, mode string, ok bool) {
	f, err := os.Open(path)
	if err != nil {
		return "", "", false
	}
	defer f.Close()
	sc := bufio.NewScanner(f)
	for sc.Scan() {
		line := strings.TrimSpace(sc.Text())
		if after, found := strings.CutPrefix(line, "lifecycle:"); found {
			lifecycle = strings.TrimSpace(strings.Split(after, "#")[0])
		}
		if after, found := strings.CutPrefix(line, "mode:"); found {
			mode = strings.TrimSpace(strings.Split(after, "#")[0])
		}
	}
	return lifecycle, mode, true
}
