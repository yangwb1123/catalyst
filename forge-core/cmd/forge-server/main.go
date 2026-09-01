// Command forge-server exposes the local Forge Workspace App Server boundary.
package main

import (
	"context"
	"os"
	"os/signal"
	"syscall"
)

var (
	serverVersion = "dev"
	serverCommit  = ""
)

func main() {
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	os.Exit(run(ctx, os.Args[1:], os.Stdout, os.Stderr))
}
