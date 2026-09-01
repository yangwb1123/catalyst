package main

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"

	"forgeos/forge-core/internal/appserver"
)

func run(ctx context.Context, args []string, stdout, stderr io.Writer) int {
	flags := flag.NewFlagSet("forge-server", flag.ContinueOnError)
	flags.SetOutput(stderr)
	listen := flags.String("listen", appserver.DefaultListenAddress, "literal loopback host:port")
	stateDir := flags.String("state-dir", "", "absolute private App Server state directory (required)")
	showVersion := flags.Bool("version", false, "print build version and exit")
	if err := flags.Parse(args); err != nil {
		if errors.Is(err, flag.ErrHelp) {
			return 0
		}
		return 2
	}
	if flags.NArg() != 0 {
		fmt.Fprintln(stderr, "forge-server: positional arguments are not supported")
		return 2
	}
	if *showVersion {
		printVersion(stdout)
		return 0
	}
	config := appserver.Config{
		ListenAddress: *listen, StateDir: *stateDir,
		Build: appserver.BuildInfo{Version: serverVersion, Commit: serverCommit},
	}
	if err := config.Validate(); err != nil {
		fmt.Fprintf(stderr, "forge-server: %v\n", err)
		return 2
	}
	if err := appserver.Run(ctx, config, announce(stdout)); err != nil {
		fmt.Fprintf(stderr, "forge-server: %v\n", err)
		return 1
	}
	return 0
}

func announce(output io.Writer) appserver.Announce {
	return func(ready appserver.Ready) error {
		return json.NewEncoder(output).Encode(ready)
	}
}

func printVersion(output io.Writer) {
	if serverCommit == "" {
		fmt.Fprintf(output, "forge-server %s\n", serverVersion)
		return
	}
	fmt.Fprintf(output, "forge-server %s (%s)\n", serverVersion, serverCommit)
}
