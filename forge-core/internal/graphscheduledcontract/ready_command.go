package graphscheduledcontract

import (
	"encoding/json"
	"errors"
	"flag"
	"io"

	"forgeos/forge-core/internal/scheduledterminal"
)

const readyCommandUsage = `usage:
  forge graph-scheduled-ready-nodes --control FILE|- --schedule-sha256 SHA256
    [--predecessor-receipt FILE|-]...

warning:
  The wave-parallel planning view: lists every topologically-ready successor
  node for the consumed receipt set, in serial order, as a JSON array of
  node IDs. It grants no authority; each listed node is materialized through
  forge graph-scheduled-node-contract (per-node candidates) and admitted by
  the runtime before any dispatch.
`

// ReadyCommand runs the effect-free wave-parallel planning CLI adapter: it
// prints the ready successor node IDs for a consumed receipt set.
func ReadyCommand(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	options, err := parseReadyCommandOptions(args)
	if errors.Is(err, flag.ErrHelp) {
		_, _ = io.WriteString(stderr, readyCommandUsage)
		return 0
	}
	if err != nil {
		return commandFailure(stderr, 2, "invalid arguments")
	}
	snapshot, err := readControl(options.control, stdin)
	if err != nil {
		return commandFailure(stderr, 1, "invalid control snapshot")
	}
	var receipts []scheduledterminal.Receipt
	if len(options.predecessorSources) > 0 {
		receipts, err = readPredecessorReceipts(options.predecessorSources, stdin)
		if err != nil {
			return commandFailure(stderr, 1, "invalid predecessor receipt")
		}
	}
	ready, err := ReadySuccessorNodes(snapshot, options.scheduleSHA256, receipts)
	if err != nil {
		return commandFailure(stderr, 1, "cannot compute ready successor nodes")
	}
	encoded, err := json.Marshal(ready)
	if err != nil {
		return commandFailure(stderr, 1, "cannot encode ready successor nodes")
	}
	encoded = append(encoded, '\n')
	written, err := stdout.Write(encoded)
	if err != nil || written != len(encoded) {
		return commandFailure(stderr, 1, "cannot write ready successor nodes")
	}
	return 0
}

type readyCommandOptions struct {
	control            string
	scheduleSHA256     string
	predecessorSources []string
}

func parseReadyCommandOptions(args []string) (readyCommandOptions, error) {
	var options readyCommandOptions
	seen := make(map[string]bool)
	flags := flag.NewFlagSet("graph-scheduled-ready-nodes", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	bindStringFlag(flags, seen, "control", &options.control)
	bindStringFlag(flags, seen, "schedule-sha256", &options.scheduleSHA256)
	bindRepeatStringFlag(flags, "predecessor-receipt", &options.predecessorSources)
	if err := flags.Parse(args); err != nil {
		return options, err
	}
	valid := flags.NArg() == 0 && len(seen) == 2 && options.control != "" &&
		isLowerHexDigest(options.scheduleSHA256) && readyCommandStdinSourceCount(options) <= 1
	if !valid {
		return options, errInvalidCandidate
	}
	return options, nil
}

func readyCommandStdinSourceCount(options readyCommandOptions) int {
	count := 0
	for _, source := range append([]string{options.control}, options.predecessorSources...) {
		if source == "-" {
			count++
		}
	}
	return count
}
