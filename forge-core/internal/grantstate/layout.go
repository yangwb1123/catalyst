package grantstate

const (
	LedgerFile = "issuance-ledger.v1.json"
	LockFile   = "issuance.lock"

	usageLedgerFile = "usage-ledger.v1.json"
	usageLockFile   = "usage.lock"
)

type stateLayout struct {
	ledger string
	lock   string
}

var (
	issuanceLayout = stateLayout{ledger: LedgerFile, lock: LockFile}
	usageLayout    = stateLayout{ledger: usageLedgerFile, lock: usageLockFile}
)

func (layout stateLayout) valid() bool {
	return layout == issuanceLayout || layout == usageLayout
}
