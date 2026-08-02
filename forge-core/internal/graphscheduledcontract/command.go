package graphscheduledcontract

import (
	"errors"
	"flag"
	"fmt"
	"io"
	"os"

	"forgeos/forge-core/internal/graphdispatch"
)

const commandUsage = `usage:
  forge graph-scheduled-node-contract --control FILE|-
    --schedule-sha256 SHA256 --endpoint HTTPS_URL --model MODEL
    --max-output-tokens N --max-model-output-bytes N --max-model-events N
    --timeout-ms N --max-cost-usd-micros N
    --pricing-snapshot-sha256 SHA256 --max-result-bytes N

warning:
  The control and candidate are private. Output is an initial-node-only passive
  candidate; it grants no lifecycle, dispatch, workspace, tool, or successor authority.
`

// Command runs the effect-free scheduled initial-node candidate CLI adapter.
func Command(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	options, err := parseCommandOptions(args)
	if errors.Is(err, flag.ErrHelp) {
		_, _ = io.WriteString(stderr, commandUsage)
		return 0
	}
	if err != nil {
		return commandFailure(stderr, 2, "invalid arguments")
	}
	snapshot, err := readControl(options.control, stdin)
	if err != nil {
		return commandFailure(stderr, 1, "invalid control snapshot")
	}
	candidate, err := BuildInitial(snapshot, options.scheduleSHA256, options.execution)
	if err != nil {
		return commandFailure(stderr, 1, "cannot build scheduled node contract candidate")
	}
	encoded, err := MarshalCandidate(candidate)
	if err != nil {
		return commandFailure(stderr, 1, "cannot encode scheduled node contract candidate")
	}
	written, err := stdout.Write(encoded)
	if err != nil || written != len(encoded) {
		return commandFailure(stderr, 1, "cannot write scheduled node contract candidate")
	}
	return 0
}

func readControl(source string, stdin io.Reader) (graphdispatch.ControlSnapshot, error) {
	if source == "-" {
		return graphdispatch.DecodeControl(stdin)
	}
	file, err := os.Open(source)
	if err != nil {
		return graphdispatch.ControlSnapshot{}, errInvalidCandidate
	}
	defer file.Close()
	return graphdispatch.DecodeControl(file)
}

func commandFailure(stderr io.Writer, code int, message string) int {
	_, _ = fmt.Fprintf(stderr, "forge graph-scheduled-node-contract: %s\n", message)
	return code
}
