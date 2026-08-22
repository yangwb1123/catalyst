//go:build unix

package grantstate

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

func (s *unixSession) commit(expected Snapshot, next []byte) error {
	if err := validateCommit(expected, next, s.maxBytes); err != nil {
		return err
	}
	current, err := s.current()
	if err != nil {
		return err
	}
	if current.Present && bytes.Equal(current.Data, next) {
		return nil
	}
	if !snapshotsEqual(current, expected) {
		return newError(CodeConflict, "commit ledger", "expected image is no longer current", nil)
	}
	temp, err := s.prepareTemp(next)
	if err != nil {
		return err
	}
	renamed := false
	defer func() {
		if !renamed {
			_ = s.port.remove(s.state, temp.name)
		}
	}()
	if err := s.requireExpected(expected, next); err != nil {
		return err
	}
	if err := s.port.rename(s.state, temp.name, s.layout.ledger); err != nil {
		return uncertain("publish ledger", err)
	}
	renamed = true
	return s.finishCommit(temp, next)
}

func validateCommit(expected Snapshot, next []byte, max int64) error {
	if len(next) == 0 {
		return newError(CodeInvalid, "commit ledger", "next image must be nonempty", nil)
	}
	if int64(len(next)) > max {
		return newError(CodeInvalid, "commit ledger", "next image exceeds byte limit", nil)
	}
	if !expected.Present && len(expected.Data) != 0 {
		return newError(CodeInvalid, "commit ledger", "missing expected image must have no bytes", nil)
	}
	if expected.Present && int64(len(expected.Data)) > max {
		return newError(CodeInvalid, "commit ledger", "expected image exceeds byte limit", nil)
	}
	return nil
}

func (s *unixSession) prepareTemp(next []byte) (preparedTemp, error) {
	name, err := s.randomTempName()
	if err != nil {
		return preparedTemp{}, newError(CodePersistence, "commit ledger", "name temporary file", err)
	}
	file, err := s.port.createExclusive(s.state, name, 0o600)
	if err != nil {
		return preparedTemp{}, newError(CodePersistence, "commit ledger", "create temporary file", err)
	}
	info, err := s.writeAndCloseTemp(file, next)
	if err != nil {
		_ = file.Close()
		_ = s.port.remove(s.state, name)
		return preparedTemp{}, err
	}
	if err := verifyNamedIdentity(s.state, name, info, 0o600); err != nil {
		_ = s.port.remove(s.state, name)
		return preparedTemp{}, newError(CodePersistence, "commit ledger", "temporary identity changed", err)
	}
	data, present, err := readStateLeaf(s.state, name, s.maxBytes)
	if err != nil || !present || !bytes.Equal(data, next) {
		_ = s.port.remove(s.state, name)
		return preparedTemp{}, newError(CodePersistence, "commit ledger", "temporary image mismatch", err)
	}
	return preparedTemp{info: info, name: name}, nil
}

func (s *unixSession) writeAndCloseTemp(file *os.File, next []byte) (fs.FileInfo, error) {
	if err := s.port.writeAll(file, next); err != nil {
		return nil, newError(CodePersistence, "commit ledger", "write temporary file", err)
	}
	if err := s.port.syncFile(file); err != nil {
		return nil, newError(CodePersistence, "commit ledger", "sync temporary file", err)
	}
	info, err := validateOpenStateLeaf(file, 0o600, "temporary state leaf")
	if err != nil {
		return nil, newError(CodePersistence, "commit ledger", "temporary file rejected", err)
	}
	if err := s.port.closeFile(file); err != nil {
		return nil, newError(CodePersistence, "commit ledger", "close temporary file", err)
	}
	return info, nil
}

func (s *unixSession) requireExpected(expected Snapshot, next []byte) error {
	if err := s.verifyBindings(); err != nil {
		return err
	}
	current, err := s.current()
	if err != nil {
		return err
	}
	if current.Present && bytes.Equal(current.Data, next) {
		return newError(CodeConflict, "commit ledger", "target changed during preparation", nil)
	}
	if !snapshotsEqual(current, expected) {
		return newError(CodeConflict, "commit ledger", "expected image changed during preparation", nil)
	}
	return nil
}

func (s *unixSession) finishCommit(temp preparedTemp, next []byte) error {
	if err := s.port.syncDirectory(s.stateDirFile); err != nil {
		return uncertain("sync state directory", err)
	}
	data, present, err := readStateLeaf(s.state, s.layout.ledger, s.maxBytes)
	if err != nil || !present || !bytes.Equal(data, next) {
		return uncertain("read back published ledger", err)
	}
	if err := verifyNamedIdentity(s.state, s.layout.ledger, temp.info, 0o600); err != nil {
		return uncertain("verify published ledger identity", err)
	}
	if err := s.verifyBindings(); err != nil {
		return uncertain("verify bindings after publish", err)
	}
	return nil
}

func (s *unixSession) randomTempName() (string, error) {
	value := make([]byte, 16)
	if err := s.port.fillRandom(value); err != nil {
		return "", err
	}
	return "." + s.layout.ledger + ".tmp-" + hex.EncodeToString(value), nil
}

func uncertain(detail string, cause error) error {
	if cause == nil {
		cause = fmt.Errorf("published image did not match")
	}
	return newError(CodePersistenceUncertain, "commit ledger", detail, cause)
}
