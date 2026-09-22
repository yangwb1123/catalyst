package main

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"strings"

	"forgeos/forge-core/internal/appserver"
	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/executionprofile"
)

const (
	maxExecutionProfileBindings         = 256
	maxExecutionProfileBindingJSONBytes = 4096
)

type executionProfileBindingFlags []string

type serverOptions struct {
	listen, stateDir, runtimeExecutable, runtimeStateDir                            string
	snaplinkIssuer, snaplinkAudience, snaplinkJWKSURL                               string
	snaplinkIntrospectURL, snaplinkIntrospectClientID, snaplinkIntrospectSecretFile string
	expectedTenant, expectedSubject, tlsCert, tlsKey                                string
	browserOrigins                                                                  string
	deviceFabricActivationFile                                                      string
	deviceInventoryLifecycleRegistryFile                                            string
	deviceClientInstanceSessionViewFile                                             string
	executionProfileBindings                                                        executionProfileBindingFlags
	showVersion                                                                     bool
}

func (bindings *executionProfileBindingFlags) Set(value string) error {
	if len(*bindings) >= maxExecutionProfileBindings || len(value) > maxExecutionProfileBindingJSONBytes {
		return fmt.Errorf("execution profile binding exceeds the configured limit")
	}
	*bindings = append(*bindings, value)
	return nil
}

func (bindings *executionProfileBindingFlags) String() string {
	return ""
}

func run(ctx context.Context, args []string, stdout, stderr io.Writer) int {
	flags := flag.NewFlagSet("forge-server", flag.ContinueOnError)
	flags.SetOutput(stderr)
	options := serverOptions{listen: appserver.DefaultListenAddress}
	flags.StringVar(&options.listen, "listen", options.listen, "literal loopback or private-interface host:port")
	flags.StringVar(&options.stateDir, "state-dir", "", "absolute private App Server state directory (required)")
	flags.StringVar(&options.runtimeExecutable, "runtime-executable", "", "absolute direct forge-runtime executable (enables session API)")
	flags.StringVar(&options.runtimeStateDir, "runtime-state-dir", "", "absolute Runtime Hub state directory (required for session API)")
	flags.StringVar(&options.snaplinkIssuer, "snaplink-issuer", "", "HTTPS Snaplink issuer URL (required for session API)")
	flags.StringVar(&options.snaplinkAudience, "snaplink-audience", "", "fixed Snaplink access-token audience (required for session API)")
	flags.StringVar(&options.snaplinkJWKSURL, "snaplink-jwks-url", "", "optional same-origin HTTPS Snaplink JWKS URL")
	flags.StringVar(&options.snaplinkIntrospectURL, "snaplink-introspect-url", "", "opt-in same-origin HTTPS Snaplink token introspection URL")
	flags.StringVar(&options.snaplinkIntrospectClientID, "snaplink-introspect-client-id", "", "resource-server client ID for Snaplink introspection")
	flags.StringVar(&options.snaplinkIntrospectSecretFile, "snaplink-introspect-secret-file", "", "owner-private file containing the Snaplink introspection client secret")
	flags.StringVar(&options.expectedTenant, "snaplink-tenant", "", "exact Coordinator tenant (required with session API)")
	flags.StringVar(&options.expectedSubject, "snaplink-subject", "", "exact Coordinator account subject (required with session API)")
	flags.StringVar(&options.tlsCert, "tls-cert", "", "TLS certificate PEM (required for private-interface listener)")
	flags.StringVar(&options.tlsKey, "tls-key", "", "private TLS key PEM (required for private-interface listener)")
	flags.StringVar(&options.browserOrigins, "browser-origins", "", "comma-separated exact HTTPS origins allowed to call the session API")
	flags.StringVar(&options.deviceFabricActivationFile, "device-fabric-activation-file", "", "owner-private device Fabric activation manifest; absent keeps Fabric OFF")
	flags.StringVar(&options.deviceInventoryLifecycleRegistryFile, "device-inventory-lifecycle-registry-file", "", "owner-private lifecycle registry image required by accepted inventory, observe, or execute admission activation")
	flags.StringVar(&options.deviceClientInstanceSessionViewFile, "device-client-instance-session-view-file", "", "owner-private client-instance/session declaration image for accepted inventory, observe, or execute admission activation")
	flags.Var(&options.executionProfileBindings, "execution-profile-binding",
		"repeatable server-owned Project profile JSON binding; profile fields are never caller-selected")
	flags.BoolVar(&options.showVersion, "version", false, "print build version and exit")
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
	return runOptions(ctx, options, stdout, stderr)
}

func runOptions(ctx context.Context, options serverOptions, stdout, stderr io.Writer) int {
	if options.showVersion {
		printVersion(stdout)
		return 0
	}
	profileBindings, err := executionprofile.ParseBindingConfigs(options.executionProfileBindings)
	if err != nil {
		fmt.Fprintln(stderr, "forge-server: invalid execution profile policy")
		return 2
	}
	var deviceFabricActivation *devicefabricgate.Request
	if options.deviceFabricActivationFile != "" {
		manifest, loadErr := devicefabricgate.LoadManifestFile(options.deviceFabricActivationFile)
		if loadErr != nil {
			fmt.Fprintf(stderr, "forge-server: device Fabric activation manifest: %v\n", loadErr)
			return 2
		}
		request := manifest.Request()
		deviceFabricActivation = &request
	}
	config := appserver.Config{
		ListenAddress: options.listen, StateDir: options.stateDir,
		RuntimeExecutable: options.runtimeExecutable, RuntimeStateDir: options.runtimeStateDir,
		SnaplinkIssuer: options.snaplinkIssuer, SnaplinkAudience: options.snaplinkAudience,
		SnaplinkJWKSURL: options.snaplinkJWKSURL, ExpectedTenantID: options.expectedTenant,
		SnaplinkIntrospectURL:                options.snaplinkIntrospectURL,
		SnaplinkIntrospectClientID:           options.snaplinkIntrospectClientID,
		SnaplinkIntrospectSecretFile:         options.snaplinkIntrospectSecretFile,
		ExpectedSubjectID:                    options.expectedSubject,
		ExecutionProfiles:                    profileBindings,
		DeviceFabricActivation:               deviceFabricActivation,
		DeviceInventoryLifecycleRegistryFile: options.deviceInventoryLifecycleRegistryFile,
		DeviceClientInstanceSessionViewFile:  options.deviceClientInstanceSessionViewFile,
		TLSCertificateFile:                   options.tlsCert, TLSPrivateKeyFile: options.tlsKey,
		Build: appserver.BuildInfo{Version: serverVersion, Commit: serverCommit},
	}
	if options.browserOrigins != "" {
		config.BrowserOrigins = strings.Split(options.browserOrigins, ",")
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
