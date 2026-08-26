package graphscheduledreconcile

import (
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
)

const commandUsage = `usage:
  forge graph-scheduled-reconcile --snapshot FILE|-
  forge graph-scheduled-reconcile --protocol-version

warning:
  The snapshot is private durable identity metadata. This command is
  effect-free and grants no execution, retry, consent, provider, workspace,
  persistence, dispatch, or successor authority.
`

// Command runs the zero-effect scheduled Graph reconciliation CLI adapter.
func Command(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	mode, source, err := parseCommandOptions(args)
	if errors.Is(err, flag.ErrHelp) {
		_, _ = io.WriteString(stderr, commandUsage)
		return 0
	}
	if err != nil {
		return commandFailure(stderr, 2, "invalid arguments")
	}
	if mode == "protocol" {
		return writeExact(stdout, stderr, []byte("1"), "cannot write protocol version")
	}
	snapshot, err := readSnapshot(source, stdin)
	if err != nil {
		return commandFailure(stderr, 1, "invalid progress snapshot")
	}
	decision, err := Reconcile(snapshot)
	if err != nil {
		return commandFailure(stderr, 1, "cannot reconcile progress snapshot")
	}
	encoded, err := MarshalDecision(decision)
	if err != nil {
		return commandFailure(stderr, 1, "cannot encode reconcile decision")
	}
	return writeExact(stdout, stderr, encoded, "cannot write reconcile decision")
}

func parseCommandOptions(args []string) (string, string, error) {
	if len(args) == 1 && args[0] == "--protocol-version" {
		return "protocol", "", nil
	}
	for _, argument := range args {
		if argument == "--protocol-version" {
			return "", "", errInvalidSnapshot
		}
	}
	var source string
	seen := false
	flags := flag.NewFlagSet("graph-scheduled-reconcile", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	flags.Func("snapshot", "", func(value string) error {
		if seen {
			return errInvalidSnapshot
		}
		seen, source = true, value
		return nil
	})
	if err := flags.Parse(args); err != nil {
		return "", "", err
	}
	if flags.NArg() != 0 || !seen || source == "" {
		return "", "", errInvalidSnapshot
	}
	return "snapshot", source, nil
}

func readSnapshot(source string, stdin io.Reader) (ProgressSnapshot, error) {
	if source == "-" {
		return DecodeSnapshot(stdin)
	}
	file, err := os.Open(source)
	if err != nil {
		return ProgressSnapshot{}, errInvalidSnapshot
	}
	defer func() { _ = file.Close() }()
	return DecodeSnapshot(file)
}

func writeExact(stdout, stderr io.Writer, data []byte, message string) int {
	written, err := stdout.Write(data)
	if err != nil || written != len(data) {
		return commandFailure(stderr, 1, message)
	}
	return 0
}

func commandFailure(stderr io.Writer, code int, message string) int {
	_, _ = fmt.Fprintf(stderr, "forge graph-scheduled-reconcile: %s\n", message)
	return code
}
