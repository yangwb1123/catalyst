package graphscheduledcontract

import (
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"unicode/utf8"

	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/scheduledterminal"
)

const commandUsage = `usage:
  forge graph-scheduled-node-contract --control FILE|-
    --schedule-sha256 SHA256 --endpoint HTTPS_URL --model MODEL
    --max-output-tokens N --max-model-output-bytes N --max-model-events N
    --timeout-ms N --max-cost-usd-micros N
    --pricing-snapshot-sha256 SHA256 --max-result-bytes N
    [--predecessor-receipt FILE|-]...

warning:
  The control and candidate are private. Without --predecessor-receipt the
  output is an initial-node-only passive candidate. With one or more receipts
  it is a successor candidate. Neither grants successor authority, and the
  candidate grants no lifecycle, dispatch, workspace, or tool authority.
`

// Command runs the effect-free scheduled candidate CLI adapter.
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
	var candidate ScheduledNodeContractCandidate
	if len(options.predecessorSources) == 0 {
		candidate, err = BuildInitial(snapshot, options.scheduleSHA256, options.execution)
	} else {
		receipts, readErr := readPredecessorReceipts(options.predecessorSources, stdin)
		if readErr != nil {
			return commandFailure(stderr, 1, "invalid predecessor receipt")
		}
		var predecessorContent string
		if options.predecessorContentSource != "" {
			content, readErr := readPredecessorContent(options.predecessorContentSource, stdin)
			if readErr != nil {
				return commandFailure(stderr, 1, "invalid predecessor content")
			}
			predecessorContent = content
		}
		candidate, err = BuildSuccessor(
			snapshot, options.scheduleSHA256, options.execution, receipts, predecessorContent,
		)
	}
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

// readPredecessorContent reads one bounded exact UTF-8 predecessor result
// text; it is embedded verbatim into the successor user Prompt.
func readPredecessorContent(source string, stdin io.Reader) (string, error) {
	var data []byte
	var err error
	if source == "-" {
		data, err = io.ReadAll(io.LimitReader(stdin, 1024*1024+1))
	} else {
		data, err = os.ReadFile(source)
	}
	if err != nil || len(data) == 0 || len(data) > 1024*1024 {
		return "", errInvalidCandidate
	}
	if !utf8.Valid(data) {
		return "", errInvalidCandidate
	}
	return string(data), nil
}

// readPredecessorReceipts reads and strictly decodes every predecessor
// receipt in ordinal order; at most one source may use stdin.
func readPredecessorReceipts(
	sources []string,
	stdin io.Reader,
) ([]scheduledterminal.Receipt, error) {
	stdinUsed := false
	receipts := make([]scheduledterminal.Receipt, 0, len(sources))
	for _, source := range sources {
		var data []byte
		var err error
		if source == "-" {
			if stdinUsed {
				return nil, errInvalidCandidate
			}
			stdinUsed = true
			data, err = io.ReadAll(io.LimitReader(stdin, 64*1024+1))
		} else {
			data, err = os.ReadFile(source)
		}
		if err != nil || len(data) == 0 || len(data) > 64*1024 {
			return nil, errInvalidCandidate
		}
		receipt, decodeErr := scheduledterminal.DecodeReceipt(data)
		if decodeErr != nil {
			return nil, errInvalidCandidate
		}
		receipts = append(receipts, receipt)
	}
	return receipts, nil
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
