//go:build unix

package authenticatedadrapprovalauthority

import (
	"bytes"
	"encoding/hex"
	"fmt"
	"io/fs"
	"os"
)

type preparedTemp struct {
	info fs.FileInfo
	name string
}

func (s *unixState) commit(expected stateSnapshot, next []byte) error {
	if err := validateCommit(expected, next, s.maxBytes); err != nil {
		return err
	}
	current, err := s.current()
	if err != nil {
		return err
	}
	if !snapshotsEqual(current, expected) {
		return errStateConflict
	}
	if current.Present && bytes.Equal(current.Data, next) {
		return nil
	}
	temporary, err := s.prepareTemp(next)
	if err != nil {
		return err
	}
	renamed := false
	defer func() {
		if !renamed {
			_ = s.port.remove(s.state, temporary.name)
		}
	}()
	if err := s.port.beforeRename(s.state, temporary.name); err != nil {
		return err
	}
	if err := s.requireExpected(expected, temporary, next); err != nil {
		return err
	}
	if err := s.port.rename(s.state, temporary.name, stateLedgerFile); err != nil {
		return uncertain("publish approval ledger", err)
	}
	renamed = true
	return s.finishCommit(temporary, next)
}

func validateCommit(expected stateSnapshot, next []byte, maximum int64) error {
	if len(next) == 0 || int64(len(next)) > maximum {
		return fmt.Errorf("next approval ledger image is empty or oversized")
	}
	if !expected.Present && len(expected.Data) != 0 {
		return fmt.Errorf("missing expected image contains bytes")
	}
	if expected.Present && int64(len(expected.Data)) > maximum {
		return fmt.Errorf("expected approval ledger image is oversized")
	}
	return nil
}

func (s *unixState) prepareTemp(next []byte) (preparedTemp, error) {
	name, err := s.randomTempName()
	if err != nil {
		return preparedTemp{}, err
	}
	file, err := s.port.createExclusive(s.state, name, privateMode)
	if err != nil {
		return preparedTemp{}, err
	}
	info, err := s.writeAndCloseTemp(file, next)
	if err != nil {
		_ = file.Close()
		_ = s.port.remove(s.state, name)
		return preparedTemp{}, err
	}
	if err := verifyNamedIdentity(s.state, name, info, privateMode); err != nil {
		_ = s.port.remove(s.state, name)
		return preparedTemp{}, err
	}
	data, present, err := readStateLeaf(s.state, name, s.maxBytes)
	if err != nil || !present || !bytes.Equal(data, next) {
		_ = s.port.remove(s.state, name)
		return preparedTemp{}, fmt.Errorf("temporary approval ledger mismatch")
	}
	return preparedTemp{info: info, name: name}, nil
}

func (s *unixState) writeAndCloseTemp(file *os.File, next []byte) (fs.FileInfo, error) {
	if err := s.port.writeAll(file, next); err != nil {
		return nil, err
	}
	if err := s.port.syncFile(file); err != nil {
		return nil, err
	}
	info, err := validateOpenLeaf(file, privateMode, "temporary approval state")
	if err != nil {
		return nil, err
	}
	if err := s.port.closeFile(file); err != nil {
		return nil, err
	}
	return info, nil
}

func (s *unixState) requireExpected(expected stateSnapshot, temporary preparedTemp,
	next []byte,
) error {
	if err := s.verifyBindings(); err != nil {
		return err
	}
	current, err := s.current()
	if err != nil {
		return err
	}
	if current.Present && bytes.Equal(current.Data, next) {
		return fmt.Errorf("%w: target changed during preparation", errStateConflict)
	}
	if !snapshotsEqual(current, expected) {
		return fmt.Errorf("%w: expected image changed during preparation", errStateConflict)
	}
	if err := s.verifyPreparedTemp(temporary, next); err != nil {
		return err
	}
	return nil
}

func (s *unixState) verifyPreparedTemp(temporary preparedTemp, next []byte) error {
	if err := verifyNamedIdentity(s.state, temporary.name, temporary.info,
		privateMode); err != nil {
		return fmt.Errorf("temporary approval ledger binding changed: %w", err)
	}
	data, present, err := readStateLeaf(s.state, temporary.name, s.maxBytes)
	if err != nil || !present || !bytes.Equal(data, next) {
		clearBytes(data)
		return fmt.Errorf("temporary approval ledger changed before publish")
	}
	clearBytes(data)
	if err := verifyNamedIdentity(s.state, temporary.name, temporary.info,
		privateMode); err != nil {
		return fmt.Errorf("temporary approval ledger binding changed: %w", err)
	}
	return nil
}

func (s *unixState) finishCommit(temporary preparedTemp, next []byte) error {
	if err := s.port.syncDirectory(s.stateDir); err != nil {
		return uncertain("sync approval state directory", err)
	}
	data, present, err := readStateLeaf(s.state, stateLedgerFile, s.maxBytes)
	if err != nil || !present || !bytes.Equal(data, next) {
		return uncertain("strictly reopen approval ledger", err)
	}
	if err := verifyNamedIdentity(s.state, stateLedgerFile, temporary.info,
		privateMode); err != nil {
		return uncertain("verify published approval ledger identity", err)
	}
	if err := s.verifyBindings(); err != nil {
		return uncertain("verify protected bindings after publish", err)
	}
	return nil
}

func (s *unixState) randomTempName() (string, error) {
	random := make([]byte, 16)
	if err := s.port.fillRandom(random); err != nil {
		return "", err
	}
	return "." + stateLedgerFile + ".tmp-" + hex.EncodeToString(random), nil
}

func uncertain(label string, err error) error {
	if err == nil {
		err = fmt.Errorf("published image did not match")
	}
	return fmt.Errorf("%w: %s: %v", errStateUncertain, label, err)
}
