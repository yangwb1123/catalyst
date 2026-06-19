package main

// forge migrate — run modes.yml's central-knob ACTION, the "startup ->
// enterprise" state transition (explorer -> engineering). It distills the
// migration with internal/migrate (the single source of the Plan), PRINTS the
// governance upgrade, and — only on --apply — flips <root>/.agent/project.yml's
// mode to engineering and INJECTS the five derived backfill tasks into
// <root>/.agent/ROADMAP.md (the same gap -> roadmap move evolve makes).
//
// FAIL-SAFE: the default is DRY — it prints the Plan and writes NOTHING. Only an
// explicit --apply mutates files, and even then it does the MINIMAL change: a
// single-line replace of project.yml's `mode:` (every other line/comment is
// preserved) and an APPEND to ROADMAP.md. Nothing here executes the derived
// tasks — doing the backfill work is a later build/evolve pass over the injected
// roadmap items.
//
// HONESTY: v1 supports only `--to engineering` (explorer -> engineering), the
// ONE migration modes.yml declares; modes.yml marks it `trigger: manual` (v3 may
// auto-fire it from lifecycle -> production).

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/migrate"
)

// migrateOpts holds the parsed `forge migrate` flags.
type migrateOpts struct {
	to    string // target mode; v1 supports only "engineering"
	apply bool   // false (default) = DRY (print only, write nothing); true = mutate files
	root  string // repo root (default $FORGE_REPO_ROOT or .)
}

// cmdMigrate parses flags, distills the Plan, prints it, and — only with
// --apply — mutates project.yml + ROADMAP.md. Returns 0 on success, 1 on an
// apply IO error, 2 on a flag/usage error, composing with run()'s dispatch like
// the other subcommands.
func cmdMigrate(args []string) int {
	fs := flag.NewFlagSet("migrate", flag.ContinueOnError)
	var o migrateOpts
	fs.StringVar(&o.to, "to", "", "target mode for the migration (v1: engineering — i.e. explorer->engineering)")
	fs.BoolVar(&o.apply, "apply", false, "APPLY the migration (mutate project.yml + inject ROADMAP tasks); default is dry (print only, write nothing)")
	fs.StringVar(&o.root, "root", "", "repo root (default $FORGE_REPO_ROOT or .)")
	if err := fs.Parse(args); err != nil {
		return 2
	}
	plan, err := planFor(o.to)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge migrate: %v\n", err)
		usage()
		return 2
	}
	o.root = gate.RepoRoot(o.root)
	printPlan(plan, o)
	if !o.apply {
		fmt.Println("\nforge migrate: DRY run — nothing written. Re-run with --apply to mutate project.yml + inject ROADMAP tasks.")
		return 0
	}
	if err := applyPlan(plan, o.root); err != nil {
		fmt.Fprintf(os.Stderr, "forge migrate: apply failed: %v\n", err)
		return 1
	}
	return 0
}

// planFor resolves the requested transition to a distilled Plan. v1 declares a
// SINGLE migration (explorer -> engineering), so only `--to engineering` is
// valid; anything else (including the empty default) is an honest usage error
// naming what IS supported — never a silent no-op or a faked second migration.
func planFor(to string) (migrate.Plan, error) {
	switch to {
	case migrate.ModeEngineering:
		return migrate.ExplorerToEngineering(), nil
	case "":
		return migrate.Plan{}, fmt.Errorf("--to is required (v1 supports: engineering, i.e. explorer->engineering)")
	default:
		return migrate.Plan{}, fmt.Errorf("unsupported --to %q (v1 supports only: engineering)", to)
	}
}

// printPlan renders the distilled governance upgrade: the tightened harness, the
// raised router floor, the re-enabled workflow rigor, and the derived backfill
// tasks. This is the WHOLE output in dry mode and the preamble in apply mode, so
// the operator always sees exactly what the migration does before/while it does it.
func printPlan(p migrate.Plan, o migrateOpts) {
	fmt.Printf("forge migrate: %s -> %s  (mode=%s, trigger=manual)\n", p.From, p.To, modeString(o.apply))
	fmt.Printf("  tighten harness: gates=%s coverage>=%d%% enforce=%s (was: warn / lint+build)\n",
		strings.Join(p.TightenGates, ","), p.CoverageThreshold, p.Enforce)
	fmt.Printf("  raise router floor: -> %s (was: haiku)\n", p.RouterFloor)
	fmt.Printf("  enable workflow: discover=%s adr=%s reviewer=%s\n",
		onOff(p.DiscoverFull, "full", "skip"), boolWord(p.ADR), boolWord(p.Reviewer))
	fmt.Printf("  derive %d backfill tasks (injected into ROADMAP on --apply):\n", len(p.Tasks))
	for _, t := range p.Tasks {
		fmt.Printf("    - [%s] %s%s — %s\n", t.Priority, t.ID, gateSuffix(t.Gate), t.Title)
	}
}

// modeString labels the run mode in the banner so DRY vs APPLY is unmistakable.
func modeString(apply bool) string {
	if apply {
		return "APPLY"
	}
	return "DRY"
}

// onOff / boolWord / gateSuffix are tiny renderers keeping printPlan flat (the
// arch-check function-length budget rewards small helpers over one long printer).
func onOff(b bool, on, off string) string {
	if b {
		return on
	}
	return off
}

func boolWord(b bool) string { return onOff(b, "true", "false") }

// gateSuffix renders " (gate: X)" when a task is gate-scoped, or "" when it is
// honestly un-scoped (add-ci / add-monitoring carry no gate in modes.yml).
func gateSuffix(g string) string {
	if g == "" {
		return ""
	}
	return " (gate: " + g + ")"
}

// applyPlan performs the migration's two side effects, in order: flip the mode
// in project.yml, then inject the derived tasks into ROADMAP.md. A failure of
// either is returned (fail-loud), so a partial apply is visible — never silently
// swallowed. ONLY reached when --apply was passed (cmdMigrate gates it).
func applyPlan(p migrate.Plan, root string) error {
	if err := setProjectMode(root, p.To); err != nil {
		return err
	}
	if err := appendRoadmapTasks(root, p.Tasks); err != nil {
		return err
	}
	fmt.Printf("\nforge migrate: APPLIED — %s/.agent/project.yml mode -> %s; %d backfill tasks appended to ROADMAP.md\n",
		root, p.To, len(p.Tasks))
	fmt.Println("  (the injected tasks are remediation DEBT; running them is a later build/evolve pass, not this command)")
	return nil
}

// setProjectMode rewrites ONLY the `mode:` line of <root>/.agent/project.yml to
// the target, preserving its trailing `# comment` and every other line verbatim.
// This is the minimal-edit, zero-dep discipline projectYAMLValue reads with: no
// YAML library, no whole-file regeneration — find the one top-level `mode:` line
// and swap its value, keeping any inline comment. A missing file or absent
// `mode:` line is a hard error (the migration's whole point is to flip the mode;
// silently doing nothing would be dishonest).
func setProjectMode(root, mode string) error {
	path := filepath.Join(root, ".agent", "project.yml")
	data, err := os.ReadFile(path)
	if err != nil {
		return fmt.Errorf("read project.yml: %w", err)
	}
	lines := strings.Split(string(data), "\n")
	replaced := false
	for i, line := range lines {
		if _, ok := strings.CutPrefix(line, "mode:"); !ok {
			continue
		}
		lines[i] = "mode: " + mode + modeLineComment(line)
		replaced = true
		break // only the first top-level mode: line
	}
	if !replaced {
		return fmt.Errorf("no top-level `mode:` line in %s (cannot flip mode)", path)
	}
	return os.WriteFile(path, []byte(strings.Join(lines, "\n")), 0o644)
}

// modeLineComment preserves the inline `# ...` comment from the original mode:
// line (with its leading spacing) so the rewrite keeps project.yml's annotation,
// e.g. "# 全闸门 (explorer | balanced | engineering | cto)". Returns "" when the
// line had no comment.
func modeLineComment(line string) string {
	i := strings.IndexByte(line, '#')
	if i < 0 {
		return ""
	}
	// Re-attach the comment with a two-space gap (the project.yml convention),
	// trimming whatever spacing preceded it so the column is stable.
	return "  " + strings.TrimRight(line[i:], " \t\r")
}

// appendRoadmapTasks injects the derived backfill tasks into <root>/.agent/
// ROADMAP.md as a fresh checklist section, APPENDING (never rewriting) so the
// existing roadmap is untouched. Each task becomes an UNCHECKED item
// `- [ ] [migrate] <title> (gate: <gate>, <priority>)` — the gap -> roadmap move,
// rendered in the same `- [ ]` checklist format the roadmap already uses. A
// missing ROADMAP.md is created (MkdirAll the .agent dir first); an existing one
// is opened for append.
func appendRoadmapTasks(root string, tasks []migrate.Task) error {
	dir := filepath.Join(root, ".agent")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return fmt.Errorf("create .agent dir: %w", err)
	}
	path := filepath.Join(dir, "ROADMAP.md")
	f, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_APPEND, 0o644)
	if err != nil {
		return fmt.Errorf("open ROADMAP.md: %w", err)
	}
	defer f.Close()
	var b strings.Builder
	b.WriteString("\n## Migration backfill (explorer -> engineering)\n")
	b.WriteString("> Auto-derived remediation debt from `forge migrate`; running these is a later build/evolve pass.\n")
	for _, t := range tasks {
		b.WriteString(roadmapLine(t))
	}
	if _, err := f.WriteString(b.String()); err != nil {
		return fmt.Errorf("append to ROADMAP.md: %w", err)
	}
	return nil
}

// roadmapLine renders one derived task as a ROADMAP checklist item:
//
//   - [ ] [migrate] <title> (gate: <gate>, <priority>)   // gate-scoped task
//   - [ ] [migrate] <title> (<priority>)                 // un-scoped (no gate)
//
// The `[migrate]` tag marks the injection source; the trailing parenthetical
// carries the gate (when present) + priority so the roadmap item is self-describing.
func roadmapLine(t migrate.Task) string {
	meta := t.Priority
	if t.Gate != "" {
		meta = "gate: " + t.Gate + ", " + t.Priority
	}
	return fmt.Sprintf("- [ ] [migrate] %s (%s)\n", t.Title, meta)
}
