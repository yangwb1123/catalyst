package graphterminal

import (
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
)

const commandUsage = `usage:
  forge graph-node-terminal-receipt --control FILE|-
  forge graph-node-terminal-receipt --protocol-version

warning:
  Control and output are private artifacts containing Node prompt/result and
  execution identities. This command is effect-free and does not access a
  provider, credential, workspace, database, or Project lane.
`

// Command runs the effect-free terminal-receipt CLI adapter.
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
	control, err := readControl(source, stdin)
	if err != nil {
		return commandFailure(stderr, 1, "invalid terminal control")
	}
	receipt, err := BuildReceipt(control)
	if err != nil {
		return commandFailure(stderr, 1, "cannot build terminal receipt")
	}
	encoded, err := MarshalReceipt(receipt)
	if err != nil {
		return commandFailure(stderr, 1, "cannot encode terminal receipt")
	}
	return writeExact(stdout, stderr, encoded, "cannot write terminal receipt")
}

func parseCommandOptions(args []string) (string, string, error) {
	if len(args) == 1 && args[0] == "--protocol-version" {
		return "protocol", "", nil
	}
	for _, argument := range args {
		if argument == "--protocol-version" {
			return "", "", errInvalidControl
		}
	}
	var source string
	seen := false
	flags := flag.NewFlagSet("graph-node-terminal-receipt", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	flags.Func("control", "", func(value string) error {
		if seen {
			return errInvalidControl
		}
		seen, source = true, value
		return nil
	})
	if err := flags.Parse(args); err != nil {
		return "", "", err
	}
	if flags.NArg() != 0 || !seen || source == "" {
		return "", "", errInvalidControl
	}
	return "control", source, nil
}

func readControl(source string, stdin io.Reader) (TerminalControl, error) {
	if source == "-" {
		return DecodeControl(stdin)
	}
	file, err := os.Open(source)
	if err != nil {
		return TerminalControl{}, errInvalidControl
	}
	defer file.Close()
	return DecodeControl(file)
}

func writeExact(stdout, stderr io.Writer, data []byte, message string) int {
	written, err := stdout.Write(data)
	if err != nil || written != len(data) {
		return commandFailure(stderr, 1, message)
	}
	return 0
}

func commandFailure(stderr io.Writer, code int, message string) int {
	_, _ = fmt.Fprintf(stderr, "forge graph-node-terminal-receipt: %s\n", message)
	return code
}
