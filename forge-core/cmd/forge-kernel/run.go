package main

import (
	"flag"
	"fmt"
	"io"

	"forgeos/forge-core/internal/bootstrapgrantissuance"
	"forgeos/forge-core/internal/bootstrapreporeadexecution"
)

type issueFunc func(bootstrapgrantissuance.Config) ([]byte, error)
type executeFunc func(bootstrapreporeadexecution.Config) ([]byte, error)

func run(args []string, stdout, stderr io.Writer, issue issueFunc) int {
	return runKernel(args, stdout, stderr, issue, nil)
}

func runKernel(args []string, stdout, stderr io.Writer, issue issueFunc,
	execute executeFunc) int {
	if len(args) == 0 {
		writeStatus(stderr, "INVALID_ARGUMENTS")
		return 2
	}
	switch args[0] {
	case "issue-bootstrap":
		return runIssue(args[1:], stdout, stderr, issue)
	case "execute-bootstrap-repo-read":
		return runExecution(args[1:], stdout, stderr, execute)
	default:
		writeStatus(stderr, "INVALID_ARGUMENTS")
		return 2
	}
}

func runIssue(args []string, stdout, stderr io.Writer, issue issueFunc) int {
	if issue == nil {
		writeStatus(stderr, "INVALID_ARGUMENTS")
		return 2
	}
	config, err := parseIssueFlags(args)
	if err != nil {
		writeStatus(stderr, "INVALID_ARGUMENTS")
		return 2
	}
	output, err := issue(config)
	return deliver(output, err, stdout, stderr, safeIssueErrorCode, "OUTPUT_FAILED")
}

func safeIssueErrorCode(err error) string {
	code := bootstrapgrantissuance.ErrorCode(err)
	switch code {
	case bootstrapgrantissuance.CodeClockRejected,
		bootstrapgrantissuance.CodeIdempotencyConflict,
		bootstrapgrantissuance.CodeInvalidConfig,
		bootstrapgrantissuance.CodeIssuerKeyRejected,
		bootstrapgrantissuance.CodeLedgerRejected,
		bootstrapgrantissuance.CodePersistenceFailed,
		bootstrapgrantissuance.CodePersistenceUncertain,
		bootstrapgrantissuance.CodePolicyRejected,
		bootstrapgrantissuance.CodeRequestRejected,
		bootstrapgrantissuance.CodeSigningRejected,
		bootstrapgrantissuance.CodeStateBusy,
		bootstrapgrantissuance.CodeStateRejected,
		bootstrapgrantissuance.CodeTrustRootRejected:
		return string(code)
	default:
		return "INTERNAL_ERROR"
	}
}

func deliver(output []byte, operationErr error, stdout, stderr io.Writer,
	safeCode func(error) string, outputFailure string) int {
	if operationErr != nil {
		writeStatus(stderr, safeCode(operationErr))
		return 1
	}
	if len(output) == 0 {
		writeStatus(stderr, "INTERNAL_ERROR")
		return 1
	}
	count, err := stdout.Write(output)
	if err != nil || count != len(output) {
		writeStatus(stderr, outputFailure)
		return 1
	}
	return 0
}

func clearOutput(value []byte) {
	for index := range value {
		value[index] = 0
	}
}

func parseIssueFlags(args []string) (bootstrapgrantissuance.Config, error) {
	flags := flag.NewFlagSet("issue-bootstrap", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	var config bootstrapgrantissuance.Config
	flags.StringVar(&config.RepositoryRoot, "repository-root", "", "")
	flags.StringVar(&config.AuthorityRoot, "authority-root", "", "")
	flags.StringVar(&config.StateDir, "state-dir", "", "")
	flags.StringVar(&config.TrustRootPath, "trust-root-path", "", "")
	flags.StringVar(&config.PolicyPath, "policy-path", "", "")
	flags.StringVar(&config.RequestPath, "request-path", "", "")
	flags.StringVar(&config.IssuerSeedPath, "issuer-seed-path", "", "")
	flags.StringVar(&config.PinnedRootSHA256, "pinned-root-sha256", "", "")
	if err := flags.Parse(args); err != nil || flags.NArg() != 0 {
		return bootstrapgrantissuance.Config{}, fmt.Errorf("invalid issue-bootstrap arguments")
	}
	required := []string{config.RepositoryRoot, config.AuthorityRoot, config.StateDir,
		config.TrustRootPath, config.PolicyPath, config.RequestPath,
		config.IssuerSeedPath, config.PinnedRootSHA256}
	for _, value := range required {
		if value == "" {
			return bootstrapgrantissuance.Config{}, fmt.Errorf("all issue-bootstrap flags are required")
		}
	}
	return config, nil
}

func writeStatus(writer io.Writer, code string) {
	_, _ = fmt.Fprintf(writer, "forge-kernel: %s\n", code)
}
