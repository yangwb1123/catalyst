package planningownership

import (
	"fmt"
	"io"
	"strings"
)

const commandUsage = `usage:
  forge capability-ownership project --catalog FILE|- --mapping FILE|-
`

// Command implements the explicit-source pure projection CLI.
func Command(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	if len(args) == 0 || args[0] != "project" {
		return commandFailure(stderr, 2, "invalid subcommand")
	}
	catalogSource, mappingSource, err := parseProjectOptions(args[1:])
	if err != nil {
		return commandFailure(stderr, 2, "invalid arguments")
	}
	catalog, err := readCommandInput(catalogSource, stdin, maxCatalogSourceBytes)
	if err != nil {
		return commandFailure(stderr, 1, "catalog input rejected")
	}
	mapping, err := readCommandInput(mappingSource, stdin, maxMappingSourceBytes)
	if err != nil {
		return commandFailure(stderr, 1, "mapping input rejected")
	}
	request, err := BuildRequest(catalog, mapping)
	if err != nil {
		return commandFailure(stderr, 1, "source validation rejected")
	}
	projection, err := Project(request)
	if err != nil {
		return commandFailure(stderr, 1, "ownership projection rejected")
	}
	return writeCommandOutput(stdout, stderr, projection.encoded)
}

func parseProjectOptions(args []string) (string, string, error) {
	if len(args) != 4 {
		return "", "", fmt.Errorf("expected exactly two options")
	}
	values := make(map[string]string, 2)
	for index := 0; index < len(args); index += 2 {
		option, value := args[index], args[index+1]
		if option != "--catalog" && option != "--mapping" || value == "" ||
			strings.HasPrefix(value, "-") && value != "-" {
			return "", "", fmt.Errorf("unknown or empty option")
		}
		if _, duplicate := values[option]; duplicate {
			return "", "", fmt.Errorf("duplicate option")
		}
		values[option] = value
	}
	catalog, catalogOK := values["--catalog"]
	mapping, mappingOK := values["--mapping"]
	if !catalogOK || !mappingOK || (catalog == "-") == (mapping == "-") {
		return "", "", fmt.Errorf("exactly one input must be stdin")
	}
	return catalog, mapping, nil
}

func readCommandInput(source string, stdin io.Reader, maximum int) ([]byte, error) {
	if source == "-" {
		return readBounded(stdin, maximum)
	}
	return readRegularFile(source, maximum)
}

func readBounded(reader io.Reader, maximum int) ([]byte, error) {
	raw, err := io.ReadAll(io.LimitReader(reader, int64(maximum)+1))
	if err != nil || len(raw) == 0 || len(raw) > maximum {
		return nil, fmt.Errorf("input is unreadable or outside bounds")
	}
	return raw, nil
}

func writeCommandOutput(stdout, stderr io.Writer, raw []byte) int {
	output := append(cloneBytes(raw), '\n')
	written, err := stdout.Write(output)
	if err != nil || written != len(output) {
		return commandFailure(stderr, 1, "projection output rejected")
	}
	return 0
}

func commandFailure(stderr io.Writer, code int, message string) int {
	_, _ = fmt.Fprintf(stderr, "forge capability-ownership: %s\n", message)
	return code
}
