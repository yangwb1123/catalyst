package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"forgeos/forge-core/internal/doctor"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/memory"
	"forgeos/forge-core/internal/trace"
	"forgeos/forge-core/internal/yaml2json"
)

// This file is a THIN CLI wrapper: flag parsing + stdout/JSON formatting for
// `forge validate`, `forge status`, and `forge doctor`. All repo-state
// inspection/anomaly-detection/governance logic lives in internal/doctor.

// ── JSON types for forge status --json ──────────────────────────────────────

// statFile is a JSON-serializable file metadata struct for forge status --json.
type statFile struct {
	Path string `json:"path"`
	Size int64  `json:"size_bytes"`
	Age  string `json:"age"`
}

// statusJSON is the structured --json output for forge status.
type statusJSON struct {
	Project           string                  `json:"project"`
	DotForge          string                  `json:"dot_forge"`
	Checkpoint        *statFile               `json:"checkpoint,omitempty"`
	CheckpointHistory int                     `json:"checkpoint_history"`
	Trace             *statFile               `json:"trace,omitempty"`
	TraceBackup       *statFile               `json:"trace_backup,omitempty"`
	Memory            *statFile               `json:"memory,omitempty"`
	CheckpointCP      *checkpointSummary      `json:"checkpoint_content,omitempty"`
	Chain             *chainStatusDisplay     `json:"chain,omitempty"`
	Migration         *migrationStatusDisplay `json:"migration,omitempty"`
}

// checkpointSummary is the parsed checkpoint content for forge status --json.
type checkpointSummary struct {
	Iteration         int     `json:"iteration"`
	RoadmapCompletion float64 `json:"roadmap_completion"`
	GatesGreen        bool    `json:"gates_green"`
	Mode              string  `json:"mode"`
}

// ── forge validate ─────────────────────────────────────────────────────────

// parseYAMLFile reads a YAML file using the native Go parser, falling back to
// the Python yaml2json shim. Returns the JSON bytes, or an error if both fail.
func parseYAMLFile(root, relPath string) ([]byte, error) {
	path := filepath.Join(root, relPath)
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	val, err := yaml2json.Decode(f)
	if err == nil {
		data, marshalErr := json.Marshal(val)
		if marshalErr == nil {
			return data, nil
		}
	}
	// Fallback to Python shim.
	f.Close()
	shim := filepath.Join(root, "harness", "yaml2json.py")
	if _, err := os.Stat(shim); err != nil {
		return nil, fmt.Errorf("Go parser failed (%v) and python shim missing", err)
	}
	return exec.Command("python3", shim, path).Output()
}

func cmdValidate(args []string) int {
	fs := flag.NewFlagSet("validate", flag.ContinueOnError)
	var root string
	var models bool
	fs.StringVar(&root, "root", "", "repo root (default $FORGE_REPO_ROOT or .)")
	fs.BoolVar(&models, "models", false, "cross-model consistency check (sixth-wave multimodel drift guard)")
	if err := fs.Parse(args); err != nil {
		return 2
	}
	root = gate.RepoRoot(root)
	if models {
		return cmdValidateModels(root)
	}
	ok := validateProjectYAML(root)
	ok = validateModesYAML(root) && ok
	ok = validateWorkflows(root) && ok
	if ok {
		fmt.Println("forge validate: all checks passed")
		return 0
	}
	return 2
}

func validateProjectYAML(root string) bool {
	path := filepath.Join(root, ".agent", "project.yml")
	if _, err := os.Stat(path); os.IsNotExist(err) {
		fmt.Printf("  [SKIP] %s — not found (ok for bare repos)\n", path)
		return true
	}
	out, err := parseYAMLFile(root, ".agent/project.yml")
	if err != nil {
		fmt.Printf("  [FAIL] %s — unparseable: %v\n", path, err)
		return false
	}
	fmt.Printf("  [PASS] %s — parseable\n", path)
	content := string(out)
	if !strings.Contains(content, `"mode"`) {
		fmt.Printf("  [WARN] %s — no 'mode' field (defaults to balanced)\n", path)
	}
	if !strings.Contains(content, `"lifecycle"`) {
		fmt.Printf("  [WARN] %s — no 'lifecycle' field (defaults to mvp)\n", path)
	}
	return true
}

func validateModesYAML(root string) bool {
	path := filepath.Join(root, ".agent", "policies", "modes.yml")
	if _, err := os.Stat(path); os.IsNotExist(err) {
		fmt.Printf("  [SKIP] %s — not found\n", path)
		return true
	}
	out, err := parseYAMLFile(root, ".agent/policies/modes.yml")
	if err != nil {
		fmt.Printf("  [FAIL] %s — unparseable: %v\n", path, err)
		return false
	}
	fmt.Printf("  [PASS] %s — parseable\n", path)
	_ = out
	return true
}

func validateWorkflows(root string) bool {
	matches, err := filepath.Glob(filepath.Join(root, ".agent", "workflows", "*.yml"))
	if err != nil {
		fmt.Printf("  [FAIL] scanning workflows: %v\n", err)
		return false
	}
	if len(matches) == 0 {
		fmt.Printf("  [SKIP] no workflow files found\n")
		return true
	}
	known := knownAgents(root)
	allOK := true
	for _, wf := range matches {
		rel, _ := filepath.Rel(root, wf)
		out, err := parseYAMLFile(root, rel)
		if err != nil {
			fmt.Printf("  [FAIL] %s — unparseable: %v\n", wf, err)
			allOK = false
			continue
		}
		ok, findings := doctor.CheckWorkflowAgents(wf, out, known)
		for _, f := range findings {
			fmt.Printf("  [%s] %s\n", f.Level, f.Message)
		}
		allOK = ok && allOK
		if ok {
			fmt.Printf("  [PASS] %s — parseable\n", wf)
		}
	}
	return allOK
}

func knownAgents(root string) map[string]bool {
	known := map[string]bool{}
	files, _ := filepath.Glob(filepath.Join(root, ".agent", "agents", "*.md"))
	for _, f := range files {
		known[strings.TrimSuffix(filepath.Base(f), ".md")] = true
	}
	return known
}

// cmdValidateModels implements `forge validate --models` (sixth-wave-
// multimodel.md §方向1): gathers known agent cards/.ai/prompts templates and
// delegates per-workflow reference-checking to doctor.EvaluateWorkflowModels.
func cmdValidateModels(root string) int {
	aiTemplates := map[string]bool{}
	if files, err := filepath.Glob(filepath.Join(root, ".ai", "prompts", "*.md")); err == nil {
		for _, f := range files {
			aiTemplates[filepath.Base(f)] = true
		}
	}
	agentsCards := knownAgents(root)

	workflowFiles, _ := filepath.Glob(filepath.Join(root, ".agent", "workflows", "*.yml"))
	if len(workflowFiles) == 0 {
		fmt.Println("  [SKIP] no workflow files to validate")
		return 0
	}

	allOK := true
	for _, wf := range workflowFiles {
		rel, _ := filepath.Rel(root, wf)
		out, err := parseYAMLFile(root, rel)
		if err != nil {
			fmt.Printf("  [FAIL] %s — unparseable: %v\n", rel, err)
			allOK = false
			continue
		}
		ok, findings := doctor.EvaluateWorkflowModels(rel, out, agentsCards, aiTemplates)
		for _, f := range findings {
			fmt.Printf("  [%s] %s\n", f.Level, f.Message)
		}
		allOK = allOK && ok
	}

	if allOK {
		fmt.Println("forge validate --models: all cross-model checks passed")
		return 0
	}
	return 2
}

// ── forge memory-prune ─────────────────────────────────────────────────────

func cmdMemoryPrune(args []string) int {
	fs := flag.NewFlagSet("memory-prune", flag.ContinueOnError)
	var root string
	var keepLast int
	fs.StringVar(&root, "root", "", "repo root (default $FORGE_REPO_ROOT or .)")
	fs.IntVar(&keepLast, "keep-last", 500, "keep only the last N entries")
	if err := fs.Parse(args); err != nil {
		return 2
	}
	root = gate.RepoRoot(root)
	if err := rejectTrackedForgeControlState(root); err != nil {
		fmt.Fprintf(os.Stderr, "forge memory-prune: %v\n", err)
		return 1
	}
	path := filepath.Join(root, ".forge", "memory.jsonl")
	removed, err := memory.Prune(path, keepLast)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge memory-prune: %v\n", err)
		return 1
	}
	if removed == 0 {
		fmt.Println("forge memory-prune: nothing to prune (store within --keep-last limit)")
		return 0
	}
	fmt.Printf("forge memory-prune: removed %d entries (kept last %d)\n", removed, keepLast)
	return 0
}

// ── forge status ───────────────────────────────────────────────────────────

func cmdStatus(args []string) int {
	fs := flag.NewFlagSet("status", flag.ContinueOnError)
	var root string
	var jsonOut bool
	var governance bool
	var showHistory bool
	fs.StringVar(&root, "root", "", "repo root (default $FORGE_REPO_ROOT or .)")
	fs.BoolVar(&jsonOut, "json", false, "output as structured JSON")
	fs.BoolVar(&governance, "governance", false, "show governance asset status and ADR trigger conditions")
	fs.BoolVar(&showHistory, "history", false, "show checkpoint history chain")
	if err := fs.Parse(args); err != nil {
		return 2
	}
	root = gate.RepoRoot(root)

	if governance {
		printGovernanceReport(doctor.Governance(root))
		return 0
	}
	if err := rejectTrackedForgeControlState(root); err != nil {
		fmt.Fprintf(os.Stderr, "forge status: %v\n", err)
		return 1
	}

	snap := doctor.Status(root)
	if snap.DotForgeMissing {
		fmt.Println("forge status: no .forge directory — no runs have been executed yet")
		return 0
	}

	if showHistory {
		return cmdStatusHistory(root)
	}

	if jsonOut {
		printStatusJSON(root, snap)
		return 0
	}

	printStatusText(root, snap)
	return 0
}

// printStatusJSON renders a StatusSnapshot as forge status --json's structured output.
func printStatusJSON(root string, snap doctor.StatusSnapshot) {
	toFile := func(f doctor.FileStat) *statFile {
		if !f.Exists {
			return nil
		}
		return &statFile{Path: f.Path, Size: f.Size, Age: f.Age}
	}
	sd := statusJSON{
		Project: filepath.Base(root), DotForge: snap.DotForge,
		Checkpoint: toFile(snap.Checkpoint), CheckpointHistory: snap.CheckpointHistory,
		Trace: toFile(snap.Trace), TraceBackup: toFile(snap.TraceBackup), Memory: toFile(snap.Memory),
		Chain: chainStatusForDisplay(root), Migration: migrationStatusForDisplay(root),
	}
	if ci := snap.CheckpointInfo; ci.Found && ci.ParseOK {
		sd.CheckpointCP = &checkpointSummary{
			Iteration: ci.Iteration, RoadmapCompletion: ci.RoadmapCompletion,
			GatesGreen: ci.GatesGreen, Mode: ci.Mode,
		}
	}
	data, _ := json.MarshalIndent(sd, "", "  ")
	fmt.Println(string(data))
}

// printStatusText renders a StatusSnapshot as forge status's plain-text output.
func printStatusText(root string, snap doctor.StatusSnapshot) {
	fmt.Printf("forge status: %s\n", snap.DotForge)
	printFileLine("checkpoint", snap.Checkpoint)
	printFileLine("trace", snap.Trace)
	printFileLine("trace backup", snap.TraceBackup)
	printFileLine("memory", snap.Memory)

	ci := snap.CheckpointInfo
	if ci.Found && ci.ParseOK {
		fmt.Printf("  checkpoint: iteration=%d roadmap=%.0f%% gates=%v mode=%s\n",
			ci.Iteration, ci.RoadmapCompletion*100, ci.GatesGreen, ci.Mode)
	} else if ci.Found {
		fmt.Printf("  checkpoint: present (unreadable or incomplete)\n")
	}
	printChainStatusText(root)
	printMigrationStatusText(root)
}

// printFileLine prints one `  <label>: <path> (<size>, <age>)` status line;
// a non-existent file prints nothing.
func printFileLine(label string, f doctor.FileStat) {
	if !f.Exists {
		return
	}
	age := f.Age
	if age == "" {
		age = "unknown"
	}
	fmt.Printf("  %s: %s (%s, %s)\n", label, f.Path, f.SizeText, age)
}

// cmdStatusHistory lists forge status --history's checkpoint chain (current +
// retain=N backups): iteration, roadmap, gate state, and age per snapshot.
func cmdStatusHistory(root string) int {
	lines := doctor.HistoryLines(root)
	if len(lines) == 0 {
		fmt.Println("forge status: no checkpoint history found")
		return 0
	}
	fmt.Printf("Checkpoint history (%d snapshots):\n", len(lines))
	fmt.Printf("  %-8s %-6s %-8s %-6s %-10s %s\n", "Snapshot", "Iter", "Roadmap", "Gates", "Age", "Mode")
	for _, line := range lines {
		fmt.Println(line)
	}
	return 0
}

// printGovernanceReport renders forge status --governance's full report
// (eighth-wave-adr-decay.md §方向4): per-directory asset health, consumption,
// the ADR-0003 trigger status, and each tracked ADR's implementation status.
func printGovernanceReport(rep doctor.GovernanceReport) {
	fmt.Println("Governance assets:")
	for _, d := range rep.Dirs {
		age := d.Age
		if age == "" {
			age = "never"
		}
		fmt.Printf("  %s: %s  %d file(s) (last modified: %s)\n", d.Label, checkMark(d.Count > 0), d.Count, age)
	}
	fmt.Println("\nConsumption:")
	fmt.Println("  Direct: 1 project (this repo)")
	fmt.Println("  forge-init snapshots: unknown (no registry)")

	fmt.Println("\nADR-0003 trigger status (submodule sharing of governance assets):")
	fmt.Println("  ❌ 被治理项目 ≥ 2~3:  FALSE (1 project, need ≥2)")
	if rep.Evolving {
		fmt.Printf("  ✅ 治理资产高频演进:  TRUE (%d changes in last 30 days)\n", rep.TotalChanges)
		fmt.Println("  → 结论: 治理资产仍高频演进，但缺更多消费者来触发 submodule 化")
	} else {
		fmt.Printf("  ❌ 治理资产高频演进:  FALSE (%d changes in last 30 days)\n", rep.TotalChanges)
		fmt.Println("  → 结论: 触发条件尚未达成（缺更多消费者）")
	}

	fmt.Println("\nADR implementation:")
	for _, a := range rep.ADRs {
		fmt.Printf("  %s %s\n", a.Mark, a.Label)
	}
}

// checkMark returns "✅" when cond is true, "❌" when false.
func checkMark(cond bool) string {
	if cond {
		return "✅"
	}
	return "❌"
}

// cmdDoctorAnomaly prints forge doctor --anomaly's checkpoint-history trend
// analysis (seventh-wave-data-realism.md §方向3); doctor.Anomaly does the work.
func cmdDoctorAnomaly(root string) int {
	rep := doctor.Anomaly(root)
	if rep.NoForgeDir {
		fmt.Println("forge doctor: no .forge directory — no checkpoint history to analyze")
		return 0
	}
	if rep.NoHistory {
		fmt.Println("forge doctor --anomaly: no checkpoint history found")
		return 0
	}

	fmt.Printf("forge doctor --anomaly: checkpoint history (%d snapshots)\n", len(rep.SnapshotLines))
	for _, line := range rep.SnapshotLines {
		fmt.Println(line)
	}

	warnCount := 0
	for _, f := range rep.Findings {
		fmt.Printf("  [%s] %s\n", f.Level, f.Message)
		if f.Level == "WARN" {
			warnCount++
		}
	}
	if warnCount == 0 {
		fmt.Println("forge doctor --anomaly: no anomalies detected")
	}
	return 0
}

// quickDoctorCheck emits doctor.QuickChecks' fast pre-run health scan as
// trace events (kind="doctor"), called automatically before forge run/evolve
// (fourth-wave-architecture.md §方向2). Advisory only — never aborts the run.
func quickDoctorCheck(root string, tracer *trace.Tracer, logln func(string)) {
	if tracer == nil {
		return
	}
	for _, c := range doctor.QuickChecks(root) {
		emitTrace(tracer, trace.Event{Kind: "doctor", Name: c.Name, Status: c.Status, Detail: c.Detail}, logln)
	}
}

// cmdDoctor implements `forge doctor [--root DIR] [--anomaly]`; see
// internal/doctor.Run for the actual health checks performed.
func cmdDoctor(args []string) int {
	fs := flag.NewFlagSet("doctor", flag.ContinueOnError)
	var root string
	var anomaly bool
	fs.StringVar(&root, "root", "", "repo root (default $FORGE_REPO_ROOT or .)")
	fs.BoolVar(&anomaly, "anomaly", false, "run checkpoint history trend analysis")
	if err := fs.Parse(args); err != nil {
		return 2
	}
	root = gate.RepoRoot(root)
	if err := rejectTrackedForgeControlState(root); err != nil {
		fmt.Fprintf(os.Stderr, "forge doctor: %v\n", err)
		return 1
	}

	if anomaly {
		return cmdDoctorAnomaly(root)
	}

	rep := doctor.Run(root)
	if rep.NoForgeDir {
		fmt.Println("forge doctor: no .forge directory — first run pending (no issues found)")
		return 0
	}

	allOK := true
	for _, c := range rep.Checks {
		fmt.Println("  " + c.Line())
		allOK = allOK && c.OK
	}
	// A missing yaml2json shim is informational (native Go parser is primary), not a failure.
	if rep.YAML2JSONShimPresent {
		fmt.Println("  [PASS] harness/yaml2json.py")
	} else {
		fmt.Println("  [INFO] harness/yaml2json.py: not found (Go native parser active)")
	}

	if allOK {
		fmt.Println("forge doctor: all checks passed")
		return 0
	}
	return 1
}
