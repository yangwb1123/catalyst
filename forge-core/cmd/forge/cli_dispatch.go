package main

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"

	"forgeos/forge-core/internal/capabilityregistry"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/goimpactprescan"
	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphplan"
	"forgeos/forge-core/internal/graphpricing"
	"forgeos/forge-core/internal/graphrelease"
	"forgeos/forge-core/internal/graphschedule"
	"forgeos/forge-core/internal/graphscheduledcontract"
	"forgeos/forge-core/internal/graphscheduledreconcile"
	"forgeos/forge-core/internal/graphscheduledrelease"
	"forgeos/forge-core/internal/graphsnapshot"
	"forgeos/forge-core/internal/graphterminal"
	"forgeos/forge-core/internal/planningownership"
	"forgeos/forge-core/internal/projectsnapshot"
	"forgeos/forge-core/internal/scheduledterminal"
	"forgeos/forge-core/internal/sessionworktree"
)

// subcommands keeps run's body a short lookup under the function-line budget.
// gate/check/accept close over delegate + the harness gate.* function they wrap.
var subcommands = map[string]func([]string) int{
	"run":          cmdRun,
	"gate":         func(rest []string) int { return delegate(gate.GateWith, rest) },
	"check":        func(rest []string) int { return delegate(gate.CheckWith, rest) },
	"accept":       func(rest []string) int { return delegate(gate.AcceptWith, rest) },
	"evolve":       cmdEvolve,
	"route":        cmdRoute,
	"migrate":      cmdMigrate,
	"detect":       cmdDetect,
	"validate":     cmdValidate,
	"memory-prune": cmdMemoryPrune,
	"init":         cmdInit,
	"status":       cmdStatus,
	"scorecard":    cmdScorecard,
	"doctor":       cmdDoctor,
	"trace":        cmdTrace,
	"preflight":    cmdPreflight,
	"capability-ownership": func(rest []string) int {
		return planningownership.Command(rest, os.Stdin, os.Stdout, os.Stderr)
	},
	"project-snapshot": func(rest []string) int {
		return projectsnapshot.Command(rest, os.Stdout, os.Stderr)
	},
	"session": func(rest []string) int {
		return sessionworktree.Command(rest, os.Stdout, os.Stderr)
	},
	"capability-registry": func(rest []string) int { return capabilityregistry.Command(rest, os.Stdin, os.Stdout, os.Stderr) },
	"graph-plan":          func(rest []string) int { return graphplan.Command(rest, os.Stdin, os.Stdout, os.Stderr) },
	"go-impact-prescan":   func(rest []string) int { return goimpactprescan.Command(rest, os.Stdin, os.Stdout, os.Stderr) },
	"graph-snapshot":      func(rest []string) int { return graphsnapshot.Command(rest, os.Stdin, os.Stdout, os.Stderr) },
	"graph-node-contract": func(rest []string) int { return graphdispatch.Command(rest, os.Stdin, os.Stdout, os.Stderr) },
	"graph-node-pricing-snapshot": func(rest []string) int {
		return graphpricing.Command(rest, os.Stdout, os.Stderr)
	},
	"graph-node-dispatch-authorize": func(rest []string) int { return graphrelease.Command(rest, os.Stdin, os.Stdout, os.Stderr) },
	"graph-node-terminal-receipt":   func(rest []string) int { return graphterminal.Command(rest, os.Stdin, os.Stdout, os.Stderr) },
	"graph-scheduled-node-terminal-receipt": func(rest []string) int {
		return scheduledterminal.Command(rest, os.Stdin, os.Stdout, os.Stderr)
	},
	"graph-execution-schedule":      func(rest []string) int { return graphschedule.Command(rest, os.Stdin, os.Stdout, os.Stderr) },
	"graph-scheduled-node-contract": func(rest []string) int { return graphscheduledcontract.Command(rest, os.Stdin, os.Stdout, os.Stderr) },
	"graph-scheduled-ready-nodes": func(rest []string) int {
		return graphscheduledcontract.ReadyCommand(rest, os.Stdin, os.Stdout, os.Stderr)
	},
	"graph-scheduled-reconcile": func(rest []string) int {
		return graphscheduledreconcile.Command(rest, os.Stdin, os.Stdout, os.Stderr)
	},
	"graph-scheduled-node-dispatch-authorize": func(rest []string) int {
		return graphscheduledrelease.Command(rest, os.Stdin, os.Stdout, os.Stderr)
	},
	"graph-scheduled-ready-node-dispatch-authorize": func(rest []string) int {
		return graphscheduledrelease.ReadyCommand(rest, os.Stdin, os.Stdout, os.Stderr)
	},
	"approve":                cmdApprove,
	"reject":                 cmdReject,
	releasePinnedExecCommand: cmdReleaseExecPinned,
}

// run dispatches a subcommand and returns the process exit code, so main stays
// a one-liner and the dispatch is testable.
func run(args []string) int {
	if len(args) == 0 {
		usage()
		return 2
	}
	if args[0] == "--version" || args[0] == "version" {
		ver := forgeVersion
		if forgeCommit != "" {
			ver += " (" + forgeCommit + ")"
		}
		fmt.Printf("forge %s\n", ver)
		return 0
	}
	cmd, rest := args[0], args[1:]
	if cmd == "-h" || cmd == "--help" || cmd == "help" {
		usage()
		return 0
	}
	if handler, ok := subcommands[cmd]; ok {
		return handler(rest)
	}
	fmt.Fprintf(os.Stderr, "forge: unknown command %q\n", cmd)
	usage()
	return 2
}

func usage() {
	fmt.Fprint(os.Stderr, `forge — ForgeOS orchestration runtime (forge-core)

usage:
  forge run    <workflow> [--materiality L0|L1|L2|L3|L4] [--mode balanced] [--lifecycle mvp] [--executor dry|command] [--agent-cmd claude] [--chain] [--max-chain-stages 8] [--approved] [--root DIR]
  forge evolve <workflow> [--materiality L0|L1|L2|L3|L4] [--mode balanced] [--lifecycle mvp] [--max-iter 5] [--executor dry|command] [--resume] [--root DIR]
  forge route  [--complexity F] [--risk-score F] [--task-type T] [--risk low|medium|high|critical] [--budget F] [--scorecard PATH]
  forge migrate (--to engineering | --to-lifecycle production) [--apply] [--root DIR]
  forge init   --name <project> [--mode balanced] [--lifecycle mvp] <target-dir>
  forge approve|reject <stage> [--root DIR]
  forge detect|scorecard|validate|memory-prune|status|doctor [--root DIR]
  forge trace [--kind K] [--status S] [--model M] [--run-id ID] [--tail N] [--strict] [--root DIR]
  forge preflight <workflow> [--root DIR]
  forge capability-ownership project --catalog FILE|- --mapping FILE|-
  forge project-snapshot capture --project-id ID --run-id ID --root DIR
  forge session start --repo DIR [--base main] [--id ID] [--worktree-root DIR]
  forge session ready --worktree DIR --id ID
  forge session status --repo DIR [--id ID]
  forge session integrate-next --repo DIR [--validate-program PATH] [--validate-arg ARG ...] [--validation-timeout D] [--keep-worktree]
  forge capability-registry validate --registry FILE|-
  forge capability-registry resolve --registry FILE|- --request FILE|-
  forge graph-plan --graph-id ID --manifest-sha256 HEX [--input FILE|-]
  forge go-impact-prescan --graph-sha256 HEX --run-id ID --changed-path PATH [--changed-path PATH ...] [--input FILE|-]
  forge graph-snapshot --project-id ID --graph-sha256 HEX --run-id ID [--profile PROFILE] [--input FILE|-]
  forge graph-node-contract --control FILE|- --endpoint HTTPS_URL --model MODEL --max-output-tokens N --max-model-output-bytes N --max-model-events N --timeout-ms N --max-cost-usd-micros N --pricing-snapshot-sha256 SHA256 --max-result-bytes N
  forge graph-node-pricing-snapshot --model MODEL --input-usd-micros-per-token-unit N --output-usd-micros-per-token-unit N --max-input-tokens N
  forge graph-node-dispatch-authorize --control FILE|-
  forge graph-node-terminal-receipt --control FILE|-
  forge graph-node-terminal-receipt --protocol-version
  forge graph-scheduled-node-terminal-receipt --control FILE|-
  forge graph-scheduled-node-terminal-receipt --protocol-version
  forge graph-execution-schedule --control FILE|-
  forge graph-scheduled-node-contract --control FILE|- --schedule-sha256 SHA256 --endpoint HTTPS_URL --model MODEL --max-output-tokens N --max-model-output-bytes N --max-model-events N --timeout-ms N --max-cost-usd-micros N --pricing-snapshot-sha256 SHA256 --max-result-bytes N
  forge graph-scheduled-reconcile --snapshot FILE|-
  forge graph-scheduled-reconcile --protocol-version
  forge graph-scheduled-node-dispatch-authorize --control FILE|-
  forge graph-scheduled-ready-node-dispatch-authorize --control FILE|-
  forge graph-scheduled-ready-node-dispatch-authorize --protocol-version
  forge gate|check|accept [--root DIR] [--timeout D] [--max-output-bytes N]
`)
}

// cmdInit implements `forge init --name <project> [--mode ...] <target-dir>`.
// It delegates to the single scaffold implementation in forge-init.mjs.
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
		fmt.Fprintln(os.Stderr, "forge init: forge-init.mjs not found. Use --root to specify the ForgeOS repo root")
		return 1
	}
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
