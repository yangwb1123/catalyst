package projectsnapshot

import (
	"context"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"time"
)

const commandUsage = `usage:
  forge project-snapshot capture --project-id ID --run-id ID --root DIR
`

type commandOptions struct {
	projectID string
	root      string
	runID     string
}

// Command implements the live, local-only project snapshot capture adapter.
func Command(args []string, stdout, stderr io.Writer) int {
	return commandWith(args, os.Environ(), stdout, stderr)
}

func commandWith(args, environment []string, stdout, stderr io.Writer) int {
	options, err := parseCommandOptions(args)
	if err != nil {
		_, _ = io.WriteString(stderr, commandUsage)
		return 2
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	production, err := Capture(
		ctx, options.root, environment, options.projectID, options.runID,
	)
	if err != nil {
		return commandFailure(stderr, "capture rejected")
	}
	output := append(production.JSON(), '\n')
	written, err := stdout.Write(output)
	if err != nil || written != len(output) {
		return commandFailure(stderr, "output rejected")
	}
	return 0
}

func parseCommandOptions(args []string) (commandOptions, error) {
	if len(args) != 7 || args[0] != "capture" {
		return commandOptions{}, fmt.Errorf("invalid project snapshot subcommand")
	}
	values := make(map[string]string, 3)
	for index := 1; index < len(args); index += 2 {
		name, value := args[index], args[index+1]
		if name != "--project-id" && name != "--root" && name != "--run-id" ||
			value == "" || strings.HasPrefix(value, "-") {
			return commandOptions{}, fmt.Errorf("invalid project snapshot option")
		}
		if _, duplicate := values[name]; duplicate {
			return commandOptions{}, fmt.Errorf("duplicate project snapshot option")
		}
		values[name] = value
	}
	if len(values) != 3 {
		return commandOptions{}, fmt.Errorf("required project snapshot option is absent")
	}
	root := values["--root"]
	if !filepath.IsAbs(root) || filepath.Clean(root) != root {
		return commandOptions{}, fmt.Errorf("project snapshot root must be clean and absolute")
	}
	return commandOptions{
		projectID: values["--project-id"], root: root, runID: values["--run-id"],
	}, nil
}

func commandFailure(stderr io.Writer, message string) int {
	_, _ = fmt.Fprintf(stderr, "forge project-snapshot: %s\n", message)
	return 1
}
