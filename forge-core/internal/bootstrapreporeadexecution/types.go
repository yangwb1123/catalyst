// Package bootstrapreporeadexecution orchestrates the sole ADR-0058
// authenticated bootstrap repository-read effect profile.
package bootstrapreporeadexecution

import (
	"context"
	"io/fs"
	"os"
	"time"

	"forgeos/forge-core/internal/grantstate"
	"forgeos/forge-core/internal/pinnedreporead"
)

const (
	maxIssuanceRootBytes  = int64(256 * 1024)
	maxExecutionRootBytes = int64(256 * 1024)
	maxPolicyBytes        = int64(512 * 1024)
	maxInvocationBytes    = int64(512 * 1024)
	maxManifestBytes      = int64(256 * 1024)
	maxIssuanceLedger     = int64(16 * 1024 * 1024)
	maxUsageLedger        = int64(16 * 1024 * 1024)
	receiptSeedBytes      = int64(32)
	privateMode           = fs.FileMode(0o600)
)

type Config struct {
	RepositoryRoot            string
	AuthorityRoot             string
	StateDir                  string
	IssuanceTrustRootPath     string
	IssuanceLedgerPath        string
	ExecutionTrustRootPath    string
	ExecutionPolicyPath       string
	InvocationPath            string
	ManifestPath              string
	ReceiptSeedPath           string
	PinnedIssuanceRootSHA256  string
	PinnedExecutionRootSHA256 string
}

type stateSession interface {
	Current() (grantstate.Snapshot, error)
	Commit(grantstate.Snapshot, []byte) error
	ReadLeaf(string, int64, fs.FileMode) ([]byte, error)
	BindRepository(string) (*os.File, error)
	VerifyRepository() error
	Close() error
}

type clock interface {
	nowUnixMilli() (int64, error)
}

type dependencies struct {
	checkPlatform       func() error
	preflightRepository func(*os.File) error
	clock               clock
	monotonicNow        func() time.Time
	openState           func(grantstate.Config) (stateSession, error)
	readFiles           func(context.Context, *os.File, []pinnedreporead.ExpectedEntry,
		pinnedreporead.Limits) ([]pinnedreporead.File, error)
}
