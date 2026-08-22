package main

import (
	"os"

	"forgeos/forge-core/internal/bootstrapgrantissuance"
	"forgeos/forge-core/internal/bootstrapreporeadexecution"
)

func main() {
	os.Exit(runKernel(os.Args[1:], os.Stdout, os.Stderr,
		bootstrapgrantissuance.IssueBootstrap, bootstrapreporeadexecution.ExecuteBootstrap))
}
