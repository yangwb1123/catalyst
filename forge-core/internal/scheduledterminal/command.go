package scheduledterminal

import (
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
)

const commandUsage = `usage:
  forge graph-scheduled-node-terminal-receipt --control FILE|-
  forge graph-scheduled-node-terminal-receipt --protocol-version

warning:
  Control and output are private scheduled lifecycle artifacts. This command
  is effect-free and does not access a provider, credential, workspace,
  database, Project lane, or successor.
`

// Command validates one independent scheduled terminal control and emits one
// intermediate receipt without performing any external effect.
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
	controlBytes, err := readControl(source, stdin)
	if err != nil {
		return commandFailure(stderr, 1, "invalid scheduled terminal control")
	}
	encoded, err := BuildReceipt(controlBytes)
	if err != nil {
		return commandFailure(stderr, 1, "cannot build scheduled terminal receipt")
	}
	return writeExact(stdout, stderr, encoded, "cannot write scheduled terminal receipt")
}

func parseCommandOptions(args []string) (string, string, error) {
	if len(args) == 1 && args[0] == "--protocol-version" {
		return "protocol", "", nil
	}
	for _, argument := range args {
		if argument == "--protocol-version" {
			return "", "", errors.New("protocol flag cannot be combined")
		}
	}
	var source string
	seen := false
	flags := flag.NewFlagSet("graph-scheduled-node-terminal-receipt", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	flags.Func("control", "", func(value string) error {
		if seen || value == "" {
			return errors.New("control source is duplicated or empty")
		}
		seen, source = true, value
		return nil
	})
	if err := flags.Parse(args); err != nil {
		return "", "", err
	}
	if flags.NArg() != 0 || !seen {
		return "", "", errors.New("control source is required")
	}
	return "control", source, nil
}

func readControl(source string, stdin io.Reader) ([]byte, error) {
	if source == "-" {
		return readBounded(stdin, maxControlBytes)
	}
	file, err := os.Open(source)
	if err != nil {
		return nil, err
	}
	defer file.Close()
	return readBounded(file, maxControlBytes)
}

func readBounded(reader io.Reader, maximum int) ([]byte, error) {
	data, err := io.ReadAll(io.LimitReader(reader, int64(maximum)+1))
	if err != nil || len(data) > maximum {
		return nil, errors.New("artifact exceeds byte bound")
	}
	return data, nil
}

func writeExact(stdout, stderr io.Writer, data []byte, message string) int {
	written, err := stdout.Write(data)
	if err != nil || written != len(data) {
		return commandFailure(stderr, 1, message)
	}
	return 0
}

func commandFailure(stderr io.Writer, code int, message string) int {
	_, _ = fmt.Fprintf(stderr, "forge graph-scheduled-node-terminal-receipt: %s\n", message)
	return code
}
