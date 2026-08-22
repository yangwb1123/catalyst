package bootstrapreporeadexecution

import (
	"fmt"
	"time"

	"forgeos/forge-core/internal/grantstate"
	"forgeos/forge-core/internal/pinnedreporead"
)

type wallClock struct{}

func (wallClock) nowUnixMilli() (int64, error) { return time.Now().UnixMilli(), nil }

// ExecuteBootstrap consumes one authenticated bootstrap repo.read Grant. Raw
// repository content is returned only after a signed terminal ledger has been
// durably published and strictly reopened.
func ExecuteBootstrap(config Config) ([]byte, error) {
	deps := dependencies{checkPlatform: pinnedreporead.CheckPlatform, clock: wallClock{},
		preflightRepository: pinnedreporead.PreflightRepository,
		monotonicNow:        time.Now, openState: openUsageState, readFiles: pinnedreporead.Read}
	return executeWith(config, deps)
}

func executeWith(config Config, deps dependencies) ([]byte, error) {
	if err := validateConfig(config); err != nil {
		return nil, coded(CodeInvalidConfig, err)
	}
	if deps.checkPlatform == nil || deps.preflightRepository == nil || deps.clock == nil ||
		deps.monotonicNow == nil ||
		deps.openState == nil || deps.readFiles == nil {
		return nil, coded(CodeInvalidConfig, fmt.Errorf("runtime dependencies are unavailable"))
	}
	if err := deps.checkPlatform(); err != nil {
		return nil, coded(CodeUnsupported, err)
	}
	session, err := deps.openState(usageStateConfig(config))
	if err != nil {
		if session != nil {
			_ = session.Close()
		}
		return nil, stateError(err)
	}
	if session == nil {
		return nil, coded(CodeStateRejected, fmt.Errorf("usage state session is unavailable"))
	}
	output, runErr := executeLocked(config, deps, session)
	closeErr := session.Close()
	if runErr != nil {
		clear(output)
		return nil, runErr
	}
	if closeErr != nil {
		clear(output)
		return nil, coded(CodePersistenceUncertain, closeErr)
	}
	return output, nil
}

func openUsageState(config grantstate.Config) (stateSession, error) {
	return grantstate.OpenUsage(config)
}

func usageStateConfig(config Config) grantstate.Config {
	return grantstate.Config{AuthorityRoot: config.AuthorityRoot,
		StateDir: config.StateDir, MaxBytes: maxUsageLedger}
}
