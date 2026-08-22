package capabilityregistry

import (
	"fmt"
	"io"
	"os"
)

const commandUsage = `usage:
  forge capability-registry validate --registry FILE|-
  forge capability-registry resolve --registry FILE|- --request FILE|-
`

// Command is the explicit-input-only CLI adapter for validation and pure
// declared resolution. It does not dereference registry content refs.
func Command(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	if len(args) == 0 {
		return commandFailure(stderr, 2, "missing subcommand")
	}
	switch args[0] {
	case "validate":
		return validateCommand(args[1:], stdin, stdout, stderr)
	case "resolve":
		return resolveCommand(args[1:], stdin, stdout, stderr)
	default:
		return commandFailure(stderr, 2, "unknown subcommand")
	}
}

func validateCommand(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	registrySource, err := parseValidateOptions(args)
	if err != nil {
		return commandFailure(stderr, 2, "invalid arguments")
	}
	raw, err := readInput(registrySource, stdin, maxRegistryBytes)
	if err != nil {
		return commandFailure(stderr, 1, "registry input rejected")
	}
	if _, err := DecodeRegistry(raw); err != nil {
		return commandFailure(stderr, 1, "registry validation rejected")
	}
	return writeCommandOutput(stdout, stderr, raw, "validated registry")
}

func resolveCommand(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	registrySource, requestSource, err := parseResolveOptions(args)
	if err != nil {
		return commandFailure(stderr, 2, "invalid arguments")
	}
	registryRaw, err := readInput(registrySource, stdin, maxRegistryBytes)
	if err != nil {
		return commandFailure(stderr, 1, "registry input rejected")
	}
	requestRaw, err := readInput(requestSource, stdin, maxRequestBytes)
	if err != nil {
		return commandFailure(stderr, 1, "request input rejected")
	}
	registry, err := DecodeRegistry(registryRaw)
	if err != nil {
		return commandFailure(stderr, 1, "registry validation rejected")
	}
	request, err := DecodeRequest(requestRaw)
	if err != nil {
		return commandFailure(stderr, 1, "request validation rejected")
	}
	assessment, err := Resolve(registry, request)
	if err != nil {
		return commandFailure(stderr, 1, "declared resolution rejected")
	}
	encoded, err := canonicalJSON(assessment)
	if err != nil {
		return commandFailure(stderr, 1, "assessment encoding rejected")
	}
	return writeCommandOutput(stdout, stderr, encoded, "resolution assessment")
}

func parseValidateOptions(args []string) (string, error) {
	if len(args) != 2 || args[0] != "--registry" || args[1] == "" {
		return "", fmt.Errorf("--registry is required")
	}
	return args[1], nil
}

func parseResolveOptions(args []string) (string, string, error) {
	options, err := parseExactOptions(args, "--registry", "--request")
	if err != nil || (options["--registry"] == "-") == (options["--request"] == "-") {
		return "", "", fmt.Errorf("exactly one input must be stdin")
	}
	return options["--registry"], options["--request"], nil
}

func parseExactOptions(args []string, allowed ...string) (map[string]string, error) {
	if len(args) != len(allowed)*2 {
		return nil, fmt.Errorf("invalid option count")
	}
	expected, result := make(map[string]struct{}, len(allowed)), make(map[string]string, len(allowed))
	for _, option := range allowed {
		expected[option] = struct{}{}
	}
	for index := 0; index < len(args); index += 2 {
		option, value := args[index], args[index+1]
		if _, exists := expected[option]; !exists || value == "" {
			return nil, fmt.Errorf("unknown or empty option")
		}
		if _, repeated := result[option]; repeated {
			return nil, fmt.Errorf("repeated option")
		}
		result[option] = value
	}
	return result, nil
}

func readInput(source string, stdin io.Reader, maximum int) ([]byte, error) {
	if source == "-" {
		return readBounded(stdin, maximum)
	}
	file, err := os.Open(source)
	if err != nil {
		return nil, err
	}
	defer func() { _ = file.Close() }()
	return readBounded(file, maximum)
}

func readBounded(reader io.Reader, maximum int) ([]byte, error) {
	raw, err := io.ReadAll(io.LimitReader(reader, int64(maximum)+1))
	if err != nil || len(raw) == 0 || len(raw) > maximum {
		return nil, fmt.Errorf("input is unreadable or outside bounds")
	}
	return raw, nil
}

func writeCommandOutput(stdout io.Writer, stderr io.Writer, raw []byte, name string) int {
	output := append(append([]byte(nil), raw...), '\n')
	written, err := stdout.Write(output)
	if err != nil || written != len(output) {
		return commandFailure(stderr, 1, "cannot write "+name)
	}
	return 0
}

func commandFailure(stderr io.Writer, code int, message string) int {
	_, _ = fmt.Fprintf(stderr, "forge capability-registry: %s\n", message)
	return code
}
