package authenticatedadrlifecycleauthority

import (
	"errors"
	"io/fs"
	"os"
)

var (
	errStateBusy               = errors.New("lifecycle state is busy")
	errStateConflict           = errors.New("lifecycle state CAS conflict")
	errStateUncertain          = errors.New("lifecycle state persistence is uncertain")
	errUnsupported             = errors.New("lifecycle state host is unsupported")
	errInvalidStoredTransition = errors.New("stored transition is invalid")
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
