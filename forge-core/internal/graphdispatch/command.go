package graphdispatch

import (
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"strconv"
)

const commandUsage = `usage:
  forge graph-node-contract --control FILE|-
    --endpoint HTTPS_URL --model MODEL
    --max-output-tokens N --max-model-output-bytes N --max-model-events N
    --timeout-ms N --max-cost-usd-micros N
    --pricing-snapshot-sha256 SHA256 --max-result-bytes N
`

type commandOptions struct {
	control   string
	execution ExecutionOptions
}

// Command runs the effect-free first-node contract CLI adapter.
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
	contract, err := Build(snapshot, options.execution)
	if err != nil {
		return commandFailure(stderr, 1, "cannot build node execution contract")
	}
	encoded, err := MarshalContract(contract)
	if err != nil {
		return commandFailure(stderr, 1, "cannot encode node execution contract")
	}
	written, err := stdout.Write(encoded)
	if err != nil || written != len(encoded) {
		return commandFailure(stderr, 1, "cannot write node execution contract")
	}
	return 0
}

func parseCommandOptions(args []string) (commandOptions, error) {
	var options commandOptions
	seen := make(map[string]bool)
	flags := flag.NewFlagSet("graph-node-contract", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	bindStringFlag(flags, seen, "control", &options.control)
	bindStringFlag(flags, seen, "endpoint", &options.execution.Endpoint)
	bindStringFlag(flags, seen, "model", &options.execution.Model)
	bindBudgetFlags(flags, seen, &options.execution)
	if err := flags.Parse(args); err != nil {
		return commandOptions{}, err
	}
	if flags.NArg() != 0 || len(seen) != 10 ||
		options.control == "" || validateOptions(options.execution) != nil {
		return commandOptions{}, errInvalidControl
	}
	return options, nil
}

func bindBudgetFlags(
	flags *flag.FlagSet,
	seen map[string]bool,
	options *ExecutionOptions,
) {
	bindUintFlag(flags, seen, "max-output-tokens", &options.MaxOutputTokens)
	bindUintFlag(flags, seen, "max-model-output-bytes", &options.MaxModelOutputBytes)
	bindUintFlag(flags, seen, "max-model-events", &options.MaxModelEvents)
	bindUintFlag(flags, seen, "timeout-ms", &options.TimeoutMilliseconds)
	bindUintFlag(flags, seen, "max-cost-usd-micros", &options.MaxCostUSDMicros)
	bindStringFlag(flags, seen, "pricing-snapshot-sha256", &options.PricingSnapshotSHA256)
	bindUintFlag(flags, seen, "max-result-bytes", &options.MaxResultBytes)
}

func bindStringFlag(
	flags *flag.FlagSet,
	seen map[string]bool,
	name string,
	target *string,
) {
	flags.Func(name, "", func(value string) error {
		if seen[name] {
			return errInvalidControl
		}
		seen[name] = true
		*target = value
		return nil
	})
}

func bindUintFlag(
	flags *flag.FlagSet,
	seen map[string]bool,
	name string,
	target *uint64,
) {
	flags.Func(name, "", func(value string) error {
		if seen[name] {
			return errInvalidControl
		}
		parsed, err := strconv.ParseUint(value, 10, 64)
		if err != nil {
			return errInvalidControl
		}
		seen[name] = true
		*target = parsed
		return nil
	})
}

func readControl(source string, stdin io.Reader) (ControlSnapshot, error) {
	if source == "-" {
		return DecodeControl(stdin)
	}
	file, err := os.Open(source)
	if err != nil {
		return ControlSnapshot{}, errInvalidControl
	}
	defer file.Close()
	return DecodeControl(file)
}

func commandFailure(stderr io.Writer, code int, message string) int {
	_, _ = fmt.Fprintf(stderr, "forge graph-node-contract: %s\n", message)
	return code
}
