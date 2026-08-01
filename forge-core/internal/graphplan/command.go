package graphplan

import (
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
)

const commandUsage = `usage:
  forge graph-plan --graph-id ID --manifest-sha256 HEX [--input FILE|-]
`

type commandOptions struct {
	graphID     string
	manifestSHA string
	input       string
}

// Command runs the inert graph-plan CLI adapter.
func Command(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	options, err := parseCommandOptions(args)
	if errors.Is(err, flag.ErrHelp) {
		_, _ = io.WriteString(stderr, commandUsage)
		return 0
	}
	if err != nil {
		return commandFailure(stderr, 2, "invalid arguments")
	}
	spec, err := readSpec(options.input, stdin)
	if err != nil {
		return commandFailure(stderr, 1, "invalid graph spec")
	}
	plan, err := Build(spec, options.graphID, options.manifestSHA)
	if err != nil {
		return commandFailure(stderr, 1, "invalid graph plan")
	}
	encoded, err := MarshalPlan(plan)
	if err != nil {
		return commandFailure(stderr, 1, "cannot encode graph plan")
	}
	written, err := stdout.Write(encoded)
	if err != nil || written != len(encoded) {
		return commandFailure(stderr, 1, "cannot write graph plan")
	}
	return 0
}

func parseCommandOptions(args []string) (commandOptions, error) {
	var options commandOptions
	options.input = "-"
	flags := flag.NewFlagSet("graph-plan", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	bindSingleFlag(flags, "graph-id", &options.graphID, "exact prepared graph identifier")
	bindSingleFlag(flags, "manifest-sha256", &options.manifestSHA, "exact graph manifest digest")
	bindSingleFlag(flags, "input", &options.input, "authored Graph spec v1 file or - for stdin")
	if err := flags.Parse(args); err != nil {
		return commandOptions{}, err
	}
	if flags.NArg() != 0 || options.graphID == "" ||
		options.manifestSHA == "" || options.input == "" {
		return commandOptions{}, errInvalidSpec
	}
	return options, nil
}

func bindSingleFlag(flags *flag.FlagSet, name string, target *string, usage string) {
	set := false
	flags.Func(name, usage, func(value string) error {
		if set {
			return errInvalidSpec
		}
		set = true
		*target = value
		return nil
	})
}

func readSpec(source string, stdin io.Reader) (Spec, error) {
	if source == "-" {
		return Decode(stdin)
	}
	file, err := os.Open(source)
	if err != nil {
		return Spec{}, errInvalidSpec
	}
	defer file.Close()
	return Decode(file)
}

func commandFailure(stderr io.Writer, code int, message string) int {
	_, _ = fmt.Fprintf(stderr, "forge graph-plan: %s\n", message)
	return code
}
