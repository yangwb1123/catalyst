// Package bootstrapgrantissuance orchestrates the sole authenticated
// bootstrap repository-read Grant issuance flow. It performs no effect.
package bootstrapgrantissuance

import (
	"io/fs"

	"forgeos/forge-core/internal/bootstrapgrantauthority"
	"forgeos/forge-core/internal/grantstate"
)

const (
	maxTrustRootBytes = int64(256 * 1024)
	maxPolicyBytes    = int64(512 * 1024)
	maxRequestBytes   = int64(1024 * 1024)
	maxLedgerBytes    = int64(16 * 1024 * 1024)
	issuerSeedBytes   = int64(32)
	privateMode       = fs.FileMode(0o600)
)

type Config struct {
	RepositoryRoot   string
	AuthorityRoot    string
	StateDir         string
	TrustRootPath    string
	PolicyPath       string
	RequestPath      string
	IssuerSeedPath   string
	PinnedRootSHA256 string
}

type stateSession interface {
	Current() (grantstate.Snapshot, error)
	Commit(grantstate.Snapshot, []byte) error
	ReadLeaf(string, int64, fs.FileMode) ([]byte, error)
	Close() error
}

type clock interface {
	nowUnixMilli() (int64, error)
}

type dependencies struct {
	clock     clock
	openState func(grantstate.Config) (stateSession, error)
}

type authenticatedInputs struct {
	policy  *bootstrapgrantauthority.Policy
	request *bootstrapgrantauthority.Request
	trust   *bootstrapgrantauthority.Trust
}
