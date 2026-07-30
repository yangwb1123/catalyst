package main

// forge migrate — run modes.yml's central-knob ACTION, the "startup ->
// enterprise" state transition (explorer -> engineering). It distills the
// migration with internal/migrate (the single source of the Plan), PRINTS the
// governance upgrade, and — only on --apply — flips <root>/.agent/project.yml's
// mode to engineering and INJECTS the five derived backfill tasks into
// <root>/.agent/ROADMAP.md (the same gap -> roadmap move evolve makes).
//
// FAIL-SAFE: the default is DRY — it prints the Plan and writes NOTHING. Only an
// explicit --apply mutates files. Both manual and lifecycle-triggered operations
// use the same repository lock, durable intent, exact before/after images, and
// terminal receipt. Nothing here executes the derived tasks — doing the backfill
// work is a later build/evolve pass over the injected roadmap items.
//
// The legacy `--to engineering` manual path remains available. The adopted
// persistent trigger is a separate, explicit state operation:
// `--to-lifecycle production`; transient run/evolve flags never enter it.

import (
	"flag"
	"fmt"
	"os"
	"strings"

	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/migrate"
	"forgeos/forge-core/internal/migrationtxn"
)

// migrateOpts holds the parsed `forge migrate` flags.
type migrateOpts struct {
	to          string // legacy manual target mode (v1: engineering)
	toLifecycle string // persistent lifecycle promotion target (v1: production)
	apply       bool   // false (default) = DRY; true = mutate with recovery
	root        string // repo root (default $FORGE_REPO_ROOT or .)
}

type migrationStatusDisplay struct {
	Pending          bool     `json:"pending"`
	Operations       []string `json:"operations,omitempty"`
	RecoveryCommands []string `json:"recovery_commands,omitempty"`
	Error            string   `json:"error,omitempty"`
}

var migrationRecoveryCommands = []string{
	"forge migrate --to-lifecycle production --apply",
	"forge migrate --to engineering --apply",
}

// cmdMigrate parses flags, distills the Plan, prints it, and — only with
// --apply — mutates project.yml + ROADMAP.md. Returns 0 on success, 1 on an
// apply IO error, 2 on a flag/usage error, composing with run()'s dispatch like
// the other subcommands.
func cmdMigrate(args []string) int {
	fs := flag.NewFlagSet("migrate", flag.ContinueOnError)
	var o migrateOpts
	fs.StringVar(&o.to, "to", "", "target mode for the migration (v1: engineering — i.e. explorer->engineering)")
	fs.StringVar(&o.toLifecycle, "to-lifecycle", "", "persist lifecycle promotion (v1: production; explorer auto-migrates to engineering)")
	fs.BoolVar(&o.apply, "apply", false, "APPLY the migration (mutate project.yml + inject ROADMAP tasks); default is dry (print only, write nothing)")
	fs.StringVar(&o.root, "root", "", "repo root (default $FORGE_REPO_ROOT or .)")
	if err := fs.Parse(args); err != nil {
		return 2
	}
	if fs.NArg() != 0 {
		fmt.Fprintf(os.Stderr, "forge migrate: unexpected positional arguments: %s\n",
			strings.Join(fs.Args(), " "))
		return 2
	}
	if o.to != "" && o.toLifecycle != "" {
		fmt.Fprintln(os.Stderr, "forge migrate: --to and --to-lifecycle are mutually exclusive")
		return 2
	}
	if o.to == "" && o.toLifecycle == "" {
		fmt.Fprintln(os.Stderr, "forge migrate: one of --to or --to-lifecycle is required")
		return 2
	}
	var manualPlan migrate.Plan
	if o.toLifecycle != "" {
		if o.toLifecycle != migrate.LifecycleProduction {
			fmt.Fprintf(os.Stderr,
				"forge migrate: unsupported --to-lifecycle %q (v1 supports only: production)\n",
				o.toLifecycle)
			return 2
		}
	} else {
		var err error
		manualPlan, err = planFor(o.to)
		if err != nil {
			fmt.Fprintf(os.Stderr, "forge migrate: %v\n", err)
			usage()
			return 2
		}
	}
	o.root = gate.RepoRoot(o.root)
	if err := rejectTrackedForgeControlState(o.root); err != nil {
		fmt.Fprintf(os.Stderr, "forge migrate: unsafe control-state provenance: %v\n", err)
		return 1
	}
	if o.toLifecycle != "" {
		return cmdLifecyclePromotion(o)
	}
	return cmdManualModeMigration(o, manualPlan)
}

func cmdManualModeMigration(o migrateOpts, plan migrate.Plan) int {
	var (
		result migrationtxn.Result
		err    error
	)
	if o.apply {
		result, err = migrationtxn.ApplyManual(o.root)
	} else {
		result, err = migrationtxn.PreviewManual(o.root)
	}
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge migrate: mode migration failed: %v\n", err)
		return 1
	}
	printPlan(plan, o)
	fmt.Printf("forge migrate: mode status=%s, lifecycle=%s -> %s\n",
		result.Status, result.FromLifecycle, result.ToLifecycle)
	if !o.apply && result.Status == migrationtxn.StatusPlanned {
		fmt.Println("\nforge migrate: DRY run — nothing written. Re-run with --apply to persist the migration.")
	}
	return 0
}

func cmdLifecyclePromotion(o migrateOpts) int {
	if o.toLifecycle != migrate.LifecycleProduction {
		fmt.Fprintf(os.Stderr,
			"forge migrate: unsupported --to-lifecycle %q (v1 supports only: production)\n",
			o.toLifecycle)
		return 2
	}
	request := migrationtxn.Request{ToLifecycle: o.toLifecycle}
	var (
		result migrationtxn.Result
		err    error
	)
	if o.apply {
		result, err = migrationtxn.Apply(o.root, request)
	} else {
		result, err = migrationtxn.Preview(o.root, request)
	}
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge migrate: lifecycle promotion failed: %v\n", err)
		return 1
	}
	printLifecyclePromotion(result, o.apply)
	return 0
}

func printLifecyclePromotion(result migrationtxn.Result, apply bool) {
	runMode := "DRY"
	if apply {
		runMode = "APPLY"
	}
	fmt.Printf("forge migrate: lifecycle %s -> %s (mode=%s, status=%s, trigger=persistent)\n",
		result.FromLifecycle, result.ToLifecycle, runMode, result.Status)
	if result.AutoMigration {
		fmt.Printf("  auto mode migration: %s -> %s; derive %d backfill tasks\n",
			result.FromMode, result.ToMode, len(result.Tasks))
		for _, task := range result.Tasks {
			fmt.Printf("    - [%s] %s%s — %s\n",
				task.Priority, task.ID, gateSuffix(task.Gate), task.Title)
		}
	} else {
		fmt.Printf("  mode unchanged: %s; ROADMAP unchanged\n", result.ToMode)
	}
	if !apply && result.Status == migrationtxn.StatusPlanned {
		fmt.Println("forge migrate: DRY run — nothing written. Re-run with --apply to persist the promotion.")
	}
}

func migrationStatusForDisplay(root string) *migrationStatusDisplay {
	summary, err := migrationtxn.InspectState(root)
	if err != nil {
		return &migrationStatusDisplay{Error: err.Error()}
	}
	if !summary.Pending && len(summary.Operations) == 0 {
		return nil
	}
	status := &migrationStatusDisplay{
		Pending:    summary.Pending,
		Operations: summary.Operations,
	}
	if summary.Pending {
		status.RecoveryCommands = append(
			[]string(nil), migrationRecoveryCommands...,
		)
	}
	return status
}

func printMigrationStatusText(root string) {
	status := migrationStatusForDisplay(root)
	if status == nil {
		return
	}
	if status.Error != "" {
		fmt.Printf("  migration: unreadable (%s)\n", status.Error)
		return
	}
	fmt.Printf("  migration: pending=%v operations=[%s]\n",
		status.Pending, strings.Join(status.Operations, ","))
	for _, command := range status.RecoveryCommands {
		fmt.Printf("    recover: %s\n", command)
	}
}

func rejectPendingPromotion(root string) error {
	if err := rejectTrackedForgeControlState(root); err != nil {
		return fmt.Errorf("unsafe control-state provenance: %w", err)
	}
	pending, err := migrationtxn.Pending(root)
	if err != nil {
		return fmt.Errorf("inspect lifecycle promotion state: %w", err)
	}
	if pending {
		return fmt.Errorf(
			"pending migration blocks execution; recover with its matching command: " +
				"`forge migrate --to-lifecycle production --apply` or " +
				"`forge migrate --to engineering --apply`",
		)
	}
	if err := migrationtxn.ValidateExecutionState(root); err != nil {
		return fmt.Errorf("completed migration state is invalid: %w", err)
	}
	return nil
}

func rejectPendingPromotionAtEntry(command, root string) int {
	if err := rejectPendingPromotion(root); err != nil {
		fmt.Fprintf(os.Stderr, "%s: %v\n", command, err)
		return 1
	}
	return 0
}

func validateFrozenProjectSelectors(o runOpts) error {
	if err := rejectPendingPromotion(o.root); err != nil {
		return err
	}
	if o.runFlagsCaptured && !o.modeExplicit {
		current := resolveMode(runOpts{root: o.root})
		if current != o.mode {
			return fmt.Errorf("persistent mode changed before run lock: planned %q, current %q; retry", o.mode, current)
		}
	}
	if o.runFlagsCaptured && !o.lifecycleExplicit {
		current := resolveLifecycle(runOpts{root: o.root})
		if current != o.lifecycle {
			return fmt.Errorf("persistent lifecycle changed before run lock: planned %q, current %q; retry",
				o.lifecycle, current)
		}
	}
	return nil
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
