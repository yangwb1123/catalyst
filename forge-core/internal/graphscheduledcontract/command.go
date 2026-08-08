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
    [--predecessor-receipt FILE|-]... [--predecessor-content FILE|-]
    [--target-node NODE_ID]

warning:
  The control and candidate are private. Without --predecessor-receipt or
  --target-node the output is an initial-node-only passive candidate. A target
  selects a ready successor even when its direct-predecessor set is empty.
  Each passive candidate grants no lifecycle, dispatch, workspace, tool, or
  successor authority.
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
	if len(options.predecessorSources) == 0 && options.targetNode == "" {
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
			options.targetNode,
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
	data, err := readBoundedSource(source, stdin, MaxPredecessorOutputBytes)
	if err != nil || len(data) == 0 || len(data) > MaxPredecessorOutputBytes {
		return "", errInvalidCandidate
	}
	if !utf8.Valid(data) {
		return "", errInvalidCandidate
	}
	return string(data), nil
}

// readPredecessorReceipts reads and strictly decodes every predecessor
// receipt in any order (topological-ready selection); at most one
// source may use stdin.
func readPredecessorReceipts(
	sources []string,
	stdin io.Reader,
) ([]scheduledterminal.Receipt, error) {
	stdinUsed := false
	receipts := make([]scheduledterminal.Receipt, 0, len(sources))
	for _, source := range sources {
		if source == "-" {
			if stdinUsed {
				return nil, errInvalidCandidate
			}
			stdinUsed = true
		}
		data, err := readBoundedSource(source, stdin, 64*1024)
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

func readBoundedSource(source string, stdin io.Reader, maximum int) ([]byte, error) {
	if source == "-" {
		return io.ReadAll(io.LimitReader(stdin, int64(maximum)+1))
	}
	file, err := os.Open(source)
	if err != nil {
		return nil, err
	}
	data, readErr := io.ReadAll(io.LimitReader(file, int64(maximum)+1))
	closeErr := file.Close()
	if readErr != nil {
		return nil, readErr
	}
	if closeErr != nil {
		return nil, closeErr
	}
	return data, nil
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
