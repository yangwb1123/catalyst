package graphsnapshot

import (
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
)

const commandUsage = `usage:
  forge graph-snapshot --project-id ID --graph-sha256 HEX --run-id ID [--profile PROFILE] [--input FILE|-]
`

type commandOptions struct {
	graphSHA256 string
	input       string
	projectID   string
	profile     string
	runID       string
}

// Command is an explicit-input-only CLI adapter for the pure projector.
func Command(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	options, err := parseCommandOptions(args)
	if errors.Is(err, flag.ErrHelp) {
		_, _ = io.WriteString(stderr, commandUsage)
		return 0
	}
	if err != nil {
		return commandFailure(stderr, 2, "invalid arguments")
	}
	graph, err := readGraphInput(options.input, stdin)
	if err != nil {
		return commandFailure(stderr, 1, "invalid graph observation")
	}
	production, err := buildCommandProjection(graph, options)
	if err != nil {
		return commandFailure(stderr, 1, "graph snapshot projection rejected")
	}
	encoded := production.JSON()
	written, err := stdout.Write(encoded)
	if err != nil || written != len(encoded) {
		return commandFailure(stderr, 1, "cannot write graph snapshot")
	}
	return 0
}

func parseCommandOptions(args []string) (commandOptions, error) {
	options := commandOptions{input: "-", profile: profileID}
	flags := flag.NewFlagSet("graph-snapshot", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	bindSingleFlag(flags, "graph-sha256", &options.graphSHA256)
	bindSingleFlag(flags, "input", &options.input)
	bindSingleFlag(flags, "project-id", &options.projectID)
	bindSingleFlag(flags, "profile", &options.profile)
	bindSingleFlag(flags, "run-id", &options.runID)
	if err := flags.Parse(args); err != nil {
		return commandOptions{}, err
	}
	if flags.NArg() != 0 || options.graphSHA256 == "" || options.input == "" ||
		options.projectID == "" || options.runID == "" ||
		options.profile != profileID && options.profile != testSourceProfileID {
		return commandOptions{}, fmt.Errorf("required argument is absent")
	}
	return options, nil
}

func buildCommandProjection(graph []byte, options commandOptions) (*Production, error) {
	if options.profile == testSourceProfileID {
		return BuildTestSource(
			graph, options.graphSHA256, options.runID, options.projectID)
	}
	return Build(graph, options.graphSHA256, options.runID, options.projectID)
}

func bindSingleFlag(flags *flag.FlagSet, name string, target *string) {
	set := false
	flags.Func(name, "exact input binding", func(value string) error {
		if set {
			return fmt.Errorf("duplicate --%s", name)
		}
		set, *target = true, value
		return nil
	})
}

func readGraphInput(source string, stdin io.Reader) ([]byte, error) {
	if source == "-" {
		return readBoundedGraph(stdin)
	}
	file, err := os.Open(source)
	if err != nil {
		return nil, err
	}
	defer func() { _ = file.Close() }()
	return readBoundedGraph(file)
}

func readBoundedGraph(reader io.Reader) ([]byte, error) {
	raw, err := io.ReadAll(io.LimitReader(reader, maxGraphBytes+1))
	if err != nil || len(raw) == 0 || len(raw) > maxGraphBytes {
		return nil, fmt.Errorf("graph observation is unreadable or outside bounds")
	}
	return raw, nil
}

func commandFailure(stderr io.Writer, code int, message string) int {
	_, _ = fmt.Fprintf(stderr, "forge graph-snapshot: %s\n", message)
	return code
}
