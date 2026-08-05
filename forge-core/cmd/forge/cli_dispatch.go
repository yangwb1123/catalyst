package main

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"

	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphplan"
	"forgeos/forge-core/internal/graphpricing"
	"forgeos/forge-core/internal/graphrelease"
	"forgeos/forge-core/internal/graphschedule"
	"forgeos/forge-core/internal/graphscheduledcontract"
	"forgeos/forge-core/internal/graphscheduledrelease"
	"forgeos/forge-core/internal/graphterminal"
	"forgeos/forge-core/internal/scheduledterminal"
)

// subcommands keeps run's body a short lookup under the function-line budget.
// gate/check/accept close over delegate + the harness gate.* function they wrap.
var subcommands = map[string]func([]string) int{
	"run":                 cmdRun,
	"gate":                func(rest []string) int { return delegate(gate.Gate, rest) },
	"check":               func(rest []string) int { return delegate(gate.Check, rest) },
	"accept":              func(rest []string) int { return delegate(gate.Accept, rest) },
	"evolve":              cmdEvolve,
	"route":               cmdRoute,
	"migrate":             cmdMigrate,
	"detect":              cmdDetect,
	"validate":            cmdValidate,
	"memory-prune":        cmdMemoryPrune,
	"init":                cmdInit,
	"status":              cmdStatus,
	"scorecard":           cmdScorecard,
	"doctor":              cmdDoctor,
	"trace":               cmdTrace,
	"preflight":           cmdPreflight,
	"graph-plan":          func(rest []string) int { return graphplan.Command(rest, os.Stdin, os.Stdout, os.Stderr) },
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
	"graph-scheduled-node-dispatch-authorize": func(rest []string) int {
		return graphscheduledrelease.Command(rest, os.Stdin, os.Stdout, os.Stderr)
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
  forge run    <workflow> [--mode balanced] [--lifecycle mvp] [--executor dry|command] [--agent-cmd claude] [--chain] [--max-chain-stages 8] [--approved] [--root DIR]
  forge evolve <workflow> [--mode balanced] [--lifecycle mvp] [--max-iter 5] [--executor dry|command] [--resume] [--root DIR]
  forge route  [--complexity F] [--risk-score F] [--task-type T] [--risk low|medium|high|critical] [--budget F] [--scorecard PATH]
  forge migrate (--to engineering | --to-lifecycle production) [--apply] [--root DIR]
  forge init   --name <project> [--mode balanced] [--lifecycle mvp] <target-dir>
  forge approve|reject <stage> [--root DIR]
  forge detect|scorecard|validate|memory-prune|status|doctor [--root DIR]
  forge trace [--kind K] [--status S] [--model M] [--run-id ID] [--tail N] [--strict] [--root DIR]
  forge preflight <workflow> [--root DIR]
  forge graph-plan --graph-id ID --manifest-sha256 HEX [--input FILE|-]
  forge graph-node-contract --control FILE|- --endpoint HTTPS_URL --model MODEL --max-output-tokens N --max-model-output-bytes N --max-model-events N --timeout-ms N --max-cost-usd-micros N --pricing-snapshot-sha256 SHA256 --max-result-bytes N
  forge graph-node-pricing-snapshot --model MODEL --input-usd-micros-per-token-unit N --output-usd-micros-per-token-unit N --max-input-tokens N
  forge graph-node-dispatch-authorize --control FILE|-
  forge graph-node-terminal-receipt --control FILE|-
  forge graph-node-terminal-receipt --protocol-version
  forge graph-scheduled-node-terminal-receipt --control FILE|-
  forge graph-scheduled-node-terminal-receipt --protocol-version
  forge graph-execution-schedule --control FILE|-
  forge graph-scheduled-node-contract --control FILE|- --schedule-sha256 SHA256 --endpoint HTTPS_URL --model MODEL --max-output-tokens N --max-model-output-bytes N --max-model-events N --timeout-ms N --max-cost-usd-micros N --pricing-snapshot-sha256 SHA256 --max-result-bytes N
  forge graph-scheduled-node-dispatch-authorize --control FILE|-
  forge gate|check|accept [--root DIR]
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
