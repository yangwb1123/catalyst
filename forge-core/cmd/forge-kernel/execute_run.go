package main

import (
	"flag"
	"fmt"
	"io"

	"forgeos/forge-core/internal/bootstrapreporeadexecution"
)

func runExecution(args []string, stdout, stderr io.Writer, execute executeFunc) int {
	if execute == nil {
		writeStatus(stderr, "INVALID_ARGUMENTS")
		return 2
	}
	config, err := parseExecutionFlags(args)
	if err != nil {
		writeStatus(stderr, "INVALID_ARGUMENTS")
		return 2
	}
	output, err := execute(config)
	defer clearOutput(output)
	return deliver(output, err, stdout, stderr, safeExecutionErrorCode,
		"OUTPUT_DELIVERY_UNCERTAIN")
}

func parseExecutionFlags(args []string) (bootstrapreporeadexecution.Config, error) {
	flags := flag.NewFlagSet("execute-bootstrap-repo-read", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	var config bootstrapreporeadexecution.Config
	flags.StringVar(&config.RepositoryRoot, "repository-root", "", "")
	flags.StringVar(&config.AuthorityRoot, "authority-root", "", "")
	flags.StringVar(&config.StateDir, "state-dir", "", "")
	flags.StringVar(&config.IssuanceTrustRootPath, "issuance-trust-root-path", "", "")
	flags.StringVar(&config.IssuanceLedgerPath, "issuance-ledger-path", "", "")
	flags.StringVar(&config.ExecutionTrustRootPath, "execution-trust-root-path", "", "")
	flags.StringVar(&config.ExecutionPolicyPath, "execution-policy-path", "", "")
	flags.StringVar(&config.InvocationPath, "invocation-path", "", "")
	flags.StringVar(&config.ManifestPath, "manifest-path", "", "")
	flags.StringVar(&config.ReceiptSeedPath, "receipt-seed-path", "", "")
	flags.StringVar(&config.PinnedIssuanceRootSHA256, "pinned-issuance-root-sha256", "", "")
	flags.StringVar(&config.PinnedExecutionRootSHA256, "pinned-execution-root-sha256", "", "")
	if err := flags.Parse(args); err != nil || flags.NArg() != 0 || missingExecutionFlag(config) {
		return bootstrapreporeadexecution.Config{}, fmt.Errorf("invalid execution arguments")
	}
	return config, nil
}

func missingExecutionFlag(config bootstrapreporeadexecution.Config) bool {
	required := []string{config.RepositoryRoot, config.AuthorityRoot, config.StateDir,
		config.IssuanceTrustRootPath, config.IssuanceLedgerPath, config.ExecutionTrustRootPath,
		config.ExecutionPolicyPath, config.InvocationPath, config.ManifestPath,
		config.ReceiptSeedPath, config.PinnedIssuanceRootSHA256,
		config.PinnedExecutionRootSHA256}
	for _, value := range required {
		if value == "" {
			return true
		}
	}
	return false
}

func safeExecutionErrorCode(err error) string {
	code := bootstrapreporeadexecution.ErrorCode(err)
	switch code {
	case bootstrapreporeadexecution.CodeClockRejected,
		bootstrapreporeadexecution.CodeExecutionRootRejected,
		bootstrapreporeadexecution.CodeGrantConsumed,
		bootstrapreporeadexecution.CodeIdempotencyConflict,
		bootstrapreporeadexecution.CodeInvalidConfig,
		bootstrapreporeadexecution.CodeInvocationRejected,
		bootstrapreporeadexecution.CodeIssuanceLedgerRejected,
		bootstrapreporeadexecution.CodeIssuanceRootRejected,
		bootstrapreporeadexecution.CodeLedgerRejected,
		bootstrapreporeadexecution.CodeManifestRejected,
		bootstrapreporeadexecution.CodePersistenceFailed,
		bootstrapreporeadexecution.CodePersistenceUncertain,
		bootstrapreporeadexecution.CodePolicyDenied,
		bootstrapreporeadexecution.CodePolicyRejected,
		bootstrapreporeadexecution.CodeRepositoryRejected,
		bootstrapreporeadexecution.CodeSignerKeyRejected,
		bootstrapreporeadexecution.CodeStateBusy,
		bootstrapreporeadexecution.CodeStateRejected,
		bootstrapreporeadexecution.CodeUnsupported:
		return string(code)
	default:
		return "INTERNAL_ERROR"
	}
}
