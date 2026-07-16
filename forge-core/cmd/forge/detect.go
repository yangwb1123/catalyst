// detect.go — `forge detect`: project-type detection and workflow suggestion.
//
// Scans the repo root for language/test/CI indicators and maps them to the most
// appropriate ForgeOS workflow + flags. This is the v1.5 of "adaptive/dynamic loop
// assembly": instead of always requiring the user to pick a workflow manually, detect
// reads observable project signals and outputs a concrete suggested command.
//
// HONEST scope (v1.5):
//   - Detection is STRUCTURAL (file presence) + SEMANTIC (reading manifest contents).
//     Parsing is lightweight line-scanning (forge-core is zero-dep, no JSON/YAML/TOML lib).
//   - Suggestions are advisory: the user runs the suggested command, not this tool.
//   - Unknown signs or parse failures → honest "unknown"; never fabricates confident verdict.
//   - project.yml lifecycle/mode is read if present; missing fields degrade gracefully.
//   - Semantic fields (go mod path, pyproject build-backend, package.json scripts) enrich
//     the profile but NEVER change the suggestion if they are empty (backward compatible).
//
// The output format is:
//
//	forge detect: project analysis
//	  language:  <lang>
//	  lifecycle: <lifecycle>
//	  has-tests: <yes|no>
//	  has-ci:    <yes|no>
//	  go-module: <module>    (go.mod `module` directive, when detected)
//	  go-version: <ver>      (go.mod `go` directive, when detected)
//	  build-script: <yes|no>  (package.json scripts.build, when detected)
//	  test-script: <yes|no>   (package.json scripts.test, when detected)
//	  deps: <n>              (package.json dependency count, when detected)
//	  build-backend: <name>  (pyproject.toml build-system.build-backend, when detected)
//	  py-version: <ver>      (pyproject.toml project.requires-python, when detected)
//	  indicators: <list>
//	  workflow:  <name>  — <reason>
//	  command:   forge evolve .agent/workflows/<name>.yml --mode <mode> --lifecycle <lc>
//
// Parsing functions live in detect_parsers.go. Tests in detect_test.go and
// detect_parsers_test.go.
package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"strings"

	"forgeos/forge-core/internal/gate"
)

// projectProfile holds the structural and semantic facts detected about a project.
type projectProfile struct {
	Language   string // go | node | python | rust | unknown
	HasTests   bool
	HasCI      bool
	Lifecycle  string   // mvp | production — from project.yml or inferred
	Mode       string   // balanced | engineering | explorer | cto — from project.yml or default
	Indicators []string // human-readable detection hits, for transparency

	// Semantic fields. Zero-value means "not detected / not applicable".
	GoModulePath   string // go.mod `module` directive
	GoVersion      string // go.mod `go` directive
	HasBuildScript bool   // package.json scripts.build exists
	HasTestScript  bool   // package.json scripts.test exists
	DepsCount      int    // package.json dependencies + devDependencies count
	BuildBackend   string // pyproject.toml build-system.build-backend
	PythonVersion  string // pyproject.toml project.requires-python
	CrateName      string // Cargo.toml [package] name
	RustEdition    string // Cargo.toml [package] edition
}

// workflowSuggestion is the output of suggestWorkflow.
type workflowSuggestion struct {
	Workflow  string // e.g. "evolve"
	Mode      string // e.g. "balanced"
	Lifecycle string // e.g. "mvp"
	Reason    string // one-sentence why
}

// cmdDetect implements `forge detect [--root DIR] [--json]`.
func cmdDetect(args []string) int {
	fs := flag.NewFlagSet("detect", flag.ContinueOnError)
	var root string
	var jsonOut bool
	fs.StringVar(&root, "root", "", "repo root (default $FORGE_REPO_ROOT or .)")
	fs.BoolVar(&jsonOut, "json", false, "output structured JSON instead of human-readable text")
	if err := fs.Parse(args); err != nil {
		return 2
	}
	root = gate.RepoRoot(root)
	p := detectProject(root)
	s := suggestWorkflow(p)

	if jsonOut {
		return cmdDetectJSON(p, s)
	}

	fmt.Printf("forge detect: project analysis\n")
	fmt.Printf("  language:     %s\n", p.Language)
	fmt.Printf("  lifecycle:    %s\n", p.Lifecycle)
	fmt.Printf("  has-tests:    %s\n", boolStr(p.HasTests))
	fmt.Printf("  has-ci:       %s\n", boolStr(p.HasCI))
	if p.GoModulePath != "" {
		fmt.Printf("  go-module:    %s\n", p.GoModulePath)
	}
	if p.GoVersion != "" {
		fmt.Printf("  go-version:   %s\n", p.GoVersion)
	}
	if p.HasBuildScript || p.HasTestScript || p.DepsCount > 0 {
		fmt.Printf("  build-script: %s\n", boolStr(p.HasBuildScript))
		fmt.Printf("  test-script:  %s\n", boolStr(p.HasTestScript))
		fmt.Printf("  deps:         %d\n", p.DepsCount)
	}
	if p.BuildBackend != "" {
		fmt.Printf("  build-backend: %s\n", p.BuildBackend)
	}
	if p.PythonVersion != "" {
		fmt.Printf("  py-version:   %s\n", p.PythonVersion)
	}
	if p.CrateName != "" || p.RustEdition != "" {
		fmt.Printf("  crate-name:   %s\n", p.CrateName)
		fmt.Printf("  rust-edition: %s\n", p.RustEdition)
	}
	fmt.Printf("  indicators:  %s\n", strings.Join(p.Indicators, "; "))
	fmt.Printf("  workflow:    %s  — %s\n", s.Workflow, s.Reason)
	fmt.Printf("  command:     forge evolve .agent/workflows/%s.yml --mode %s --lifecycle %s\n",
		s.Workflow, s.Mode, s.Lifecycle)
	return 0
}

// cmdDetectJSON outputs the detection result as structured JSON and exits 0.
type detectJSONOutput struct {
	Language   string   `json:"language"`
	Lifecycle  string   `json:"lifecycle"`
	HasTests   bool     `json:"has_tests"`
	HasCI      bool     `json:"has_ci"`
	Indicators []string `json:"indicators"`

	// Semantic fields (omitted when empty)
	GoModulePath   string `json:"go_module_path,omitempty"`
	GoVersion      string `json:"go_version,omitempty"`
	HasBuildScript bool   `json:"has_build_script,omitempty"`
	HasTestScript  bool   `json:"has_test_script,omitempty"`
	DepsCount      int    `json:"deps_count,omitempty"`
	BuildBackend   string `json:"build_backend,omitempty"`
	PythonVersion  string `json:"python_version,omitempty"`
	CrateName      string `json:"crate_name,omitempty"`
	RustEdition    string `json:"rust_edition,omitempty"`

	// Workflow suggestion
	Workflow       string `json:"workflow"`
	WorkflowMode   string `json:"workflow_mode"`
	WorkflowLC     string `json:"workflow_lifecycle"`
	WorkflowReason string `json:"workflow_reason"`

	// Human-ready command
	Command string `json:"command"`
}

func cmdDetectJSON(p projectProfile, s workflowSuggestion) int {
	out := detectJSONOutput{
		Language:       p.Language,
		Lifecycle:      p.Lifecycle,
		HasTests:       p.HasTests,
		HasCI:          p.HasCI,
		Indicators:     p.Indicators,
		GoModulePath:   p.GoModulePath,
		GoVersion:      p.GoVersion,
		HasBuildScript: p.HasBuildScript,
		HasTestScript:  p.HasTestScript,
		DepsCount:      p.DepsCount,
		BuildBackend:   p.BuildBackend,
		PythonVersion:  p.PythonVersion,
		CrateName:      p.CrateName,
		RustEdition:    p.RustEdition,
		Workflow:       s.Workflow,
		WorkflowMode:   s.Mode,
		WorkflowLC:     s.Lifecycle,
		WorkflowReason: s.Reason,
		Command: fmt.Sprintf("forge evolve .agent/workflows/%s.yml --mode %s --lifecycle %s",
			s.Workflow, s.Mode, s.Lifecycle),
	}
	data, err := json.MarshalIndent(out, "", "  ")
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge detect: JSON output error: %v\n", err)
		return 1
	}
	fmt.Println(string(data))
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
