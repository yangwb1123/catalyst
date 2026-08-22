package sessionworktree

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"time"
)

const commandUsage = `usage:
  forge session start --repo DIR [--base main] [--id ID] [--worktree-root DIR]
  forge session ready --worktree DIR --id ID
  forge session status --repo DIR [--id ID]
  forge session integrate-next --repo DIR [--validate-program PATH] [--validate-arg ARG ...] [--validation-timeout D] [--keep-worktree]
`

type stringList []string

func (values *stringList) String() string { return fmt.Sprint([]string(*values)) }
func (values *stringList) Set(value string) error {
	*values = append(*values, value)
	return nil
}

func Command(args []string, stdout, stderr io.Writer) int {
	if len(args) == 0 {
		return commandError(stderr, 2, "missing subcommand")
	}
	switch args[0] {
	case "start":
		return startCommand(args[1:], stdout, stderr)
	case "ready":
		return readyCommand(args[1:], stdout, stderr)
	case "status":
		return statusCommand(args[1:], stdout, stderr)
	case "integrate-next":
		return integrateCommand(args[1:], stdout, stderr)
	default:
		return commandError(stderr, 2, "unknown subcommand")
	}
}

func startCommand(args []string, stdout, stderr io.Writer) int {
	flags := flag.NewFlagSet("session start", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	repo := flags.String("repo", ".", "primary repository worktree")
	base := flags.String("base", "main", "local base branch")
	id := flags.String("id", "", "session id")
	worktrees := flags.String("worktree-root", "", "external worktree parent")
	if flags.Parse(args) != nil || flags.NArg() != 0 {
		return commandError(stderr, 2, "invalid start arguments")
	}
	session, err := Start(context.Background(), StartOptions{
		RepositoryRoot: *repo, BaseBranch: *base,
		SessionID: *id, WorktreeRoot: *worktrees,
	})
	return writeSessionResult(session, err, stdout, stderr)
}

func readyCommand(args []string, stdout, stderr io.Writer) int {
	flags := flag.NewFlagSet("session ready", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	worktree := flags.String("worktree", ".", "session worktree")
	id := flags.String("id", "", "session id")
	if flags.Parse(args) != nil || flags.NArg() != 0 || *id == "" {
		return commandError(stderr, 2, "invalid ready arguments")
	}
	session, err := Ready(context.Background(), ReadyOptions{
		SessionID: *id, Worktree: *worktree,
	})
	return writeSessionResult(session, err, stdout, stderr)
}

func statusCommand(args []string, stdout, stderr io.Writer) int {
	flags := flag.NewFlagSet("session status", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	repo := flags.String("repo", ".", "repository worktree")
	id := flags.String("id", "", "session id")
	if flags.Parse(args) != nil || flags.NArg() != 0 {
		return commandError(stderr, 2, "invalid status arguments")
	}
	if *id != "" {
		session, err := Get(context.Background(), *repo, *id)
		return writeSessionResult(session, err, stdout, stderr)
	}
	sessions, err := List(context.Background(), *repo)
	return writeJSONResult(sessions, err, stdout, stderr)
}

func integrateCommand(args []string, stdout, stderr io.Writer) int {
	flags := flag.NewFlagSet("session integrate-next", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	repo := flags.String("repo", ".", "primary repository worktree")
	program := flags.String("validate-program", "", "controller-owned validation program")
	timeout := flags.Duration("validation-timeout", 3*time.Hour, "validation deadline")
	keep := flags.Bool("keep-worktree", false, "retain merged worktree and branch")
	var validationArgs stringList
	flags.Var(&validationArgs, "validate-arg", "validation argument; repeatable")
	if flags.Parse(args) != nil || flags.NArg() != 0 || *timeout <= 0 {
		return commandError(stderr, 2, "invalid integrate-next arguments")
	}
	command, err := controllerValidation(*program, validationArgs, *timeout)
	if err != nil {
		return commandError(stderr, 1, err.Error())
	}
	session, err := IntegrateNext(context.Background(), IntegrateOptions{
		RepositoryRoot: *repo, Validation: command, KeepWorktree: *keep,
	})
	if errors.Is(err, ErrNoReadySession) {
		return commandError(stderr, 3, err.Error())
	}
	return writeSessionResult(session, err, stdout, stderr)
}

func controllerValidation(program string, args []string, timeout time.Duration) (ValidationCommand, error) {
	if program != "" {
		return ValidationCommand{Program: program, Args: append([]string(nil), args...), Timeout: timeout}, nil
	}
	if len(args) != 0 {
		return ValidationCommand{}, fmt.Errorf("--validate-arg requires --validate-program")
	}
	executable, err := os.Executable()
	if err != nil {
		return ValidationCommand{}, fmt.Errorf("resolve forge executable: %w", err)
	}
	return ValidationCommand{
		Program: executable, Args: []string{"accept", "--root", ".", "--timeout", "0"},
		Timeout: timeout,
	}, nil
}

func writeSessionResult(session Session, err error, stdout, stderr io.Writer) int {
	return writeJSONResult(session, err, stdout, stderr)
}

func writeJSONResult(value any, err error, stdout, stderr io.Writer) int {
	if err != nil {
		return commandError(stderr, 1, err.Error())
	}
	encoded, err := json.Marshal(value)
	if err != nil {
		return commandError(stderr, 1, "encode result")
	}
	if _, err := stdout.Write(append(encoded, '\n')); err != nil {
		return commandError(stderr, 1, "write result")
	}
	return 0
}

func commandError(stderr io.Writer, code int, message string) int {
	_, _ = fmt.Fprintf(stderr, "forge session: %s\n%s", message, commandUsage)
	return code
}
