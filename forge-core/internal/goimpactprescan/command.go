package goimpactprescan

import (
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
)

const commandUsage = `usage:
  forge go-impact-prescan --graph-sha256 HEX --run-id ID --changed-path PATH [--changed-path PATH ...] [--input FILE|-]
`

type commandOptions struct {
	changedPaths []string
	graphSHA256  string
	input        string
	runID        string
}

// Command is the pure-bytes CLI adapter. It never invokes live capture or
// reads a repository; --input is an exact ADR-0053 graph document.
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
	production, err := Build(graph, options.graphSHA256, options.runID, options.changedPaths)
	if err != nil {
		return commandFailure(stderr, 1, "impact prescan rejected")
	}
	encoded := production.JSON()
	written, err := stdout.Write(encoded)
	if err != nil || written != len(encoded) {
		return commandFailure(stderr, 1, "cannot write impact prescan")
	}
	return 0
}

func parseCommandOptions(args []string) (commandOptions, error) {
	options := commandOptions{input: "-"}
	flags := flag.NewFlagSet("go-impact-prescan", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	bindSingleCommandFlag(flags, "graph-sha256", &options.graphSHA256)
	bindSingleCommandFlag(flags, "run-id", &options.runID)
	bindSingleCommandFlag(flags, "input", &options.input)
	flags.Func("changed-path", "canonical changed repository path", func(value string) error {
		options.changedPaths = append(options.changedPaths, value)
		return nil
	})
	if err := flags.Parse(args); err != nil {
		return commandOptions{}, err
	}
	if flags.NArg() != 0 || options.graphSHA256 == "" || options.runID == "" ||
		options.input == "" || len(options.changedPaths) == 0 {
		return commandOptions{}, fmt.Errorf("required argument is absent")
	}
	return options, nil
}

func bindSingleCommandFlag(flags *flag.FlagSet, name string, target *string) {
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
	limited := io.LimitReader(reader, maxGraphBytes+1)
	raw, err := io.ReadAll(limited)
	if err != nil || len(raw) == 0 || len(raw) > maxGraphBytes {
		return nil, fmt.Errorf("graph observation is unreadable or outside bounds")
	}
	return raw, nil
}

func commandFailure(stderr io.Writer, code int, message string) int {
	_, _ = fmt.Fprintf(stderr, "forge go-impact-prescan: %s\n", message)
	return code
}
