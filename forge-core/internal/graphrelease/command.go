package graphrelease

import (
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
)

const commandUsage = `usage:
  forge graph-node-dispatch-authorize --control FILE|-

warning:
  Output is a private authorization artifact containing Project, endpoint,
  model, destination, pricing, and request identities. Redirect it only to a
  trusted consumer. It does not release dispatch authority, send, or run anything.
`

// Command runs the effect-free dispatch-authorization CLI adapter.
func Command(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	source, err := parseCommandOptions(args)
	if errors.Is(err, flag.ErrHelp) {
		_, _ = io.WriteString(stderr, commandUsage)
		return 0
	}
	if err != nil {
		return commandFailure(stderr, 2, "invalid arguments")
	}
	control, err := readControl(source, stdin)
	if err != nil {
		return commandFailure(stderr, 1, "invalid release control")
	}
	authorization, err := BuildAuthorization(control)
	if err != nil {
		return commandFailure(stderr, 1, "cannot authorize node dispatch")
	}
	encoded, err := MarshalAuthorization(authorization)
	if err != nil {
		return commandFailure(stderr, 1, "cannot encode dispatch authorization")
	}
	written, err := stdout.Write(encoded)
	if err != nil || written != len(encoded) {
		return commandFailure(stderr, 1, "cannot write dispatch authorization")
	}
	return 0
}

func parseCommandOptions(args []string) (string, error) {
	var source string
	seen := false
	flags := flag.NewFlagSet("graph-node-dispatch-authorize", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	flags.Func("control", "", func(value string) error {
		if seen {
			return errInvalidControl
		}
		seen = true
		source = value
		return nil
	})
	if err := flags.Parse(args); err != nil {
		return "", err
	}
	if flags.NArg() != 0 || !seen || source == "" {
		return "", errInvalidControl
	}
	return source, nil
}

func readControl(source string, stdin io.Reader) (ReleaseControl, error) {
	if source == "-" {
		return DecodeControl(stdin)
	}
	file, err := os.Open(source)
	if err != nil {
		return ReleaseControl{}, errInvalidControl
	}
	defer file.Close()
	return DecodeControl(file)
}

func commandFailure(stderr io.Writer, code int, message string) int {
	_, _ = fmt.Fprintf(stderr, "forge graph-node-dispatch-authorize: %s\n", message)
	return code
}
