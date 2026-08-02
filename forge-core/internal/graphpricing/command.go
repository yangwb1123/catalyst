package graphpricing

import (
	"errors"
	"flag"
	"fmt"
	"io"
	"strconv"
)

const commandUsage = `usage:
  forge graph-node-pricing-snapshot --model MODEL
    --input-usd-micros-per-token-unit N
    --output-usd-micros-per-token-unit N
    --max-input-tokens N

The provider and endpoint are fixed by the registered destination. This command
only emits an operator-asserted local pricing artifact; it reads no credential,
constructs no provider, performs no network request, and releases no authority.
`

// Command runs the effect-free pricing-snapshot CLI adapter.
func Command(args []string, stdout, stderr io.Writer) int {
	input, err := parseCommandOptions(args)
	if errors.Is(err, flag.ErrHelp) {
		_, _ = io.WriteString(stderr, commandUsage)
		return 0
	}
	if err != nil {
		return commandFailure(stderr, 2, "invalid arguments")
	}
	value, err := Build(input)
	if err != nil {
		return commandFailure(stderr, 1, "cannot build pricing snapshot")
	}
	encoded, err := Marshal(value)
	if err != nil {
		return commandFailure(stderr, 1, "cannot encode pricing snapshot")
	}
	written, err := stdout.Write(encoded)
	if err != nil || written != len(encoded) {
		return commandFailure(stderr, 1, "cannot write pricing snapshot")
	}
	return 0
}

func parseCommandOptions(args []string) (Input, error) {
	var input Input
	seen := make(map[string]bool)
	flags := flag.NewFlagSet("graph-node-pricing-snapshot", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	bindStringFlag(flags, seen, "model", &input.Model)
	bindUintFlag(flags, seen, "input-usd-micros-per-token-unit", &input.InputUSDMicrosPerTokenUnit)
	bindUintFlag(flags, seen, "output-usd-micros-per-token-unit", &input.OutputUSDMicrosPerTokenUnit)
	bindUintFlag(flags, seen, "max-input-tokens", &input.MaxInputTokens)
	if err := flags.Parse(args); err != nil {
		return Input{}, err
	}
	if flags.NArg() != 0 || len(seen) != 4 || !validInput(input) {
		return Input{}, errInvalidSnapshot
	}
	return input, nil
}

func bindStringFlag(flags *flag.FlagSet, seen map[string]bool, name string, target *string) {
	flags.Func(name, "", func(value string) error {
		if seen[name] {
			return errInvalidSnapshot
		}
		seen[name] = true
		*target = value
		return nil
	})
}

func bindUintFlag(flags *flag.FlagSet, seen map[string]bool, name string, target *uint64) {
	flags.Func(name, "", func(value string) error {
		if seen[name] {
			return errInvalidSnapshot
		}
		parsed, err := strconv.ParseUint(value, 10, 64)
		if err != nil {
			return errInvalidSnapshot
		}
		seen[name] = true
		*target = parsed
		return nil
	})
}

func commandFailure(stderr io.Writer, code int, message string) int {
	_, _ = fmt.Fprintf(stderr, "forge graph-node-pricing-snapshot: %s\n", message)
	return code
}
