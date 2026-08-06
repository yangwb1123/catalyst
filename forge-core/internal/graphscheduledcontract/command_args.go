package graphscheduledcontract

import (
	"flag"
	"io"
	"strconv"

	"forgeos/forge-core/internal/graphdispatch"
)

type commandOptions struct {
	control                  string
	scheduleSHA256           string
	execution                graphdispatch.ExecutionOptions
	predecessorSources       []string
	predecessorContentSource string
	targetNode               string
}

func parseCommandOptions(args []string) (commandOptions, error) {
	var options commandOptions
	seen := make(map[string]bool)
	flags := flag.NewFlagSet("graph-scheduled-node-contract", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	bindStringFlag(flags, seen, "control", &options.control)
	bindStringFlag(flags, seen, "schedule-sha256", &options.scheduleSHA256)
	bindStringFlag(flags, seen, "endpoint", &options.execution.Endpoint)
	bindStringFlag(flags, seen, "model", &options.execution.Model)
	bindBudgetFlags(flags, seen, &options.execution)
	bindRepeatStringFlag(flags, "predecessor-receipt", &options.predecessorSources)
	bindOptionalStringFlag(flags, "predecessor-content", &options.predecessorContentSource)
	bindOptionalStringFlag(flags, "target-node", &options.targetNode)
	if err := flags.Parse(args); err != nil {
		return commandOptions{}, err
	}
	valid := flags.NArg() == 0 && len(seen) == 11 && options.control != "" &&
		isLowerHexDigest(options.scheduleSHA256) && validExecutionOptions(options.execution)
	if !valid {
		return commandOptions{}, errInvalidCandidate
	}
	return options, nil
}

func bindRepeatStringFlag(
	flags *flag.FlagSet,
	name string,
	target *[]string,
) {
	flags.Func(name, "", func(value string) error {
		*target = append(*target, value)
		return nil
	})
}

func bindOptionalStringFlag(
	flags *flag.FlagSet,
	name string,
	target *string,
) {
	flags.Func(name, "", func(value string) error {
		if *target != "" {
			return errInvalidCandidate
		}
		*target = value
		return nil
	})
}

func bindBudgetFlags(
	flags *flag.FlagSet,
	seen map[string]bool,
	options *graphdispatch.ExecutionOptions,
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
			return errInvalidCandidate
		}
		seen[name], *target = true, value
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
			return errInvalidCandidate
		}
		parsed, err := strconv.ParseUint(value, 10, 64)
		if err != nil {
			return errInvalidCandidate
		}
		seen[name], *target = true, parsed
		return nil
	})
}
