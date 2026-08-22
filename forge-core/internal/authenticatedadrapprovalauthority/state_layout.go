package authenticatedadrapprovalauthority

import (
	"errors"
	"io/fs"
	"os"
)

const (
	stateLedgerFile = "architecture-decision-approval-ledger.v1.json"
	stateLockFile   = "architecture-decision-approval.lock"
)

var (
	errStateBusy      = errors.New("approval state is busy")
	errStateConflict  = errors.New("approval state CAS conflict")
	errStateUncertain = errors.New("approval state persistence is uncertain")
	errUnsupported    = errors.New("approval state host is unsupported")
)

type commitPort interface {
	fillRandom([]byte) error
	createExclusive(*os.Root, string, fs.FileMode) (*os.File, error)
	writeAll(*os.File, []byte) error
	syncFile(*os.File) error
	closeFile(*os.File) error
	beforeRename(*os.Root, string) error
	rename(*os.Root, string, string) error
	syncDirectory(*os.File) error
	remove(*os.Root, string) error
}

func cloneBytes(value []byte) []byte {
	if value == nil {
		return nil
	}
	return append([]byte(nil), value...)
}

func clearBytes(value []byte) {
	for index := range value {
		value[index] = 0
	}
}

func discardBytes(value []byte) []byte {
	clearBytes(value)
	return nil
}
