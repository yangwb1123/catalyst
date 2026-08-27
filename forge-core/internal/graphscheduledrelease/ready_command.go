package graphscheduledrelease

import (
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
)

const readyCommandUsage = `usage:
  forge graph-scheduled-ready-node-dispatch-authorize --control FILE|-
  forge graph-scheduled-ready-node-dispatch-authorize --protocol-version

warning:
  Output is passive policy for at most one future release of the exact selected
  ready node. It is not consent, current execution authority, a lifecycle
  admission, lane claim, provider send, receipt, retry, or durable progress,
  and this command performs no external effect.
`

// ReadyCommand runs the zero-effect scheduled ready-node v2 adapter.
func ReadyCommand(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	mode, source, err := parseReadyCommandOptions(args)
	if errors.Is(err, flag.ErrHelp) {
		_, _ = io.WriteString(stderr, readyCommandUsage)
		return 0
	}
	if err != nil {
		return readyCommandFailure(stderr, 2, "invalid arguments")
	}
	if mode == "protocol" {
		return writeReadyExact(stdout, stderr, []byte("2"), "cannot write protocol version")
	}
	control, err := readReadyReleaseControl(source, stdin)
	if err != nil {
		return readyCommandFailure(stderr, 1, "invalid ready-node release control")
	}
	authorization, err := BuildReadyAuthorization(control)
	if err != nil {
		return readyCommandFailure(stderr, 1, "cannot authorize scheduled ready node")
	}
	encoded, err := MarshalReadyAuthorization(authorization)
	if err != nil {
		return readyCommandFailure(stderr, 1, "cannot encode ready-node authorization")
	}
	return writeReadyExact(stdout, stderr, encoded, "cannot write ready-node authorization")
}

func parseReadyCommandOptions(args []string) (string, string, error) {
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
	flags := flag.NewFlagSet("graph-scheduled-ready-node-dispatch-authorize", flag.ContinueOnError)
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

func readReadyReleaseControl(source string, stdin io.Reader) (ReadyReleaseControl, error) {
	if source == "-" {
		return DecodeReadyReleaseControl(stdin)
	}
	file, err := os.Open(source)
	if err != nil {
		return ReadyReleaseControl{}, errInvalidControl
	}
	defer func() { _ = file.Close() }()
	return DecodeReadyReleaseControl(file)
}

func writeReadyExact(stdout, stderr io.Writer, data []byte, message string) int {
	written, err := stdout.Write(data)
	if err != nil || written != len(data) {
		return readyCommandFailure(stderr, 1, message)
	}
	return 0
}

func readyCommandFailure(stderr io.Writer, code int, message string) int {
	_, _ = fmt.Fprintf(stderr, "forge graph-scheduled-ready-node-dispatch-authorize: %s\n", message)
	return code
}
