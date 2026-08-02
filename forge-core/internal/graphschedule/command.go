package graphschedule

import (
	"errors"
	"flag"
	"fmt"
	"io"
	"os"

	"forgeos/forge-core/internal/graphdispatch"
)

const commandUsage = `usage:
  forge graph-execution-schedule --control FILE|-

warning:
  The control is private. Output is a passive static schedule: it observes no
  progress and grants no execution, provider, workspace, tool, or writeback authority.
`

// Command runs the effect-free Graph execution-schedule CLI adapter.
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
		return commandFailure(stderr, 1, "invalid control snapshot")
	}
	schedule, err := Build(control)
	if err != nil {
		return commandFailure(stderr, 1, "cannot build execution schedule")
	}
	encoded, err := MarshalSchedule(schedule)
	if err != nil {
		return commandFailure(stderr, 1, "cannot encode execution schedule")
	}
	written, err := stdout.Write(encoded)
	if err != nil || written != len(encoded) {
		return commandFailure(stderr, 1, "cannot write execution schedule")
	}
	return 0
}

func parseCommandOptions(args []string) (string, error) {
	var source string
	seen := false
	flags := flag.NewFlagSet("graph-execution-schedule", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	flags.Func("control", "", func(value string) error {
		if seen {
			return errInvalidControl
		}
		seen, source = true, value
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

func readControl(source string, stdin io.Reader) (graphdispatch.ControlSnapshot, error) {
	if source == "-" {
		return graphdispatch.DecodeControl(stdin)
	}
	file, err := os.Open(source)
	if err != nil {
		return graphdispatch.ControlSnapshot{}, errInvalidControl
	}
	defer file.Close()
	return graphdispatch.DecodeControl(file)
}

func commandFailure(stderr io.Writer, code int, message string) int {
	_, _ = fmt.Fprintf(stderr, "forge graph-execution-schedule: %s\n", message)
	return code
}
