package migrationtxn

import (
	"bytes"
	"crypto/sha256"
	"fmt"
	"os"
	"path/filepath"

	"forgeos/forge-core/internal/statefs"
)

const (
	projectMaxBytes = 1 << 20
	roadmapMaxBytes = 16 << 20
	stateMaxBytes   = 48 << 20
	receiptMaxBytes = 64 << 10
)

type fileImage struct {
	Present bool   `json:"present"`
	Mode    uint32 `json:"mode"`
	Data    []byte `json:"data"`
	SHA256  string `json:"sha256"`
}

type fileOps interface {
	readTracked(path string, maxBytes int64) (fileImage, error)
	writeTracked(path string, expected, image fileImage) error
	inspectState(path string) (bool, error)
	readState(path string, maxBytes int64) ([]byte, bool, error)
	writeState(path string, data []byte) error
	removeState(path string) error
	ensureStateDir(path string) error
}

type realFileOps struct{}

func (realFileOps) readTracked(path string, maxBytes int64) (fileImage, error) {
	data, mode, present, err := statefs.ReadTracked(path, maxBytes)
	if err != nil {
		return fileImage{}, err
	}
	return newFileImage(data, mode, present), nil
}

func (realFileOps) writeTracked(path string, expected, image fileImage) error {
	if !image.Present {
		return fmt.Errorf("migrationtxn: refusing to publish an absent target image")
	}
	return statefs.AtomicWriteTrackedIfUnchanged(
		path,
		expected.Data,
		os.FileMode(expected.Mode),
		expected.Present,
		image.Data,
		os.FileMode(image.Mode),
	)
}

func (realFileOps) inspectState(path string) (bool, error) {
	_, present, err := statefs.InspectRegular(path)
	return present, err
}

func (realFileOps) readState(path string, maxBytes int64) ([]byte, bool, error) {
	return statefs.ReadRegularUnmodified(path, maxBytes)
}

func (realFileOps) writeState(path string, data []byte) error {
	return statefs.AtomicWrite(path, data, 0o600)
}

func (realFileOps) removeState(path string) error {
	if err := statefs.RemoveRegular(path); err != nil {
		return err
	}
	return statefs.SyncDir(filepath.Dir(path))
}

func (realFileOps) ensureStateDir(path string) error {
	return statefs.EnsurePrivateDir(path)
}

func newFileImage(data []byte, mode os.FileMode, present bool) fileImage {
	if !present {
		return fileImage{}
	}
	owned := make([]byte, len(data))
	copy(owned, data)
	sum := sha256.Sum256(owned)
	return fileImage{
		Present: true,
		Mode:    uint32(mode.Perm()),
		Data:    owned,
		SHA256:  fmt.Sprintf("%x", sum[:]),
	}
}

func validateFileImage(image fileImage, maxBytes int, label string) error {
	if !image.Present {
		if image.Mode != 0 || image.Data != nil || image.SHA256 != "" {
			return fmt.Errorf("%s absent image carries content", label)
		}
		return nil
	}
	if image.Data == nil {
		return fmt.Errorf("%s present image uses non-canonical null data", label)
	}
	if len(image.Data) > maxBytes {
		return fmt.Errorf("%s image exceeds %d bytes", label, maxBytes)
	}
	if image.Mode == 0 || image.Mode&^uint32(0o777) != 0 {
		return fmt.Errorf("%s image has invalid permission mode %#o", label, image.Mode)
	}
	sum := sha256.Sum256(image.Data)
	if image.SHA256 != fmt.Sprintf("%x", sum[:]) {
		return fmt.Errorf("%s image digest mismatch", label)
	}
	return nil
}

func sameFileImage(left, right fileImage) bool {
	return left.Present == right.Present &&
		left.Mode == right.Mode &&
		left.SHA256 == right.SHA256 &&
		(left.Data == nil) == (right.Data == nil) &&
		bytes.Equal(left.Data, right.Data)
}

func projectPath(root string) string {
	return filepath.Join(root, ".agent", "project.yml")
}

func roadmapPath(root string) string {
	return filepath.Join(root, ".agent", "ROADMAP.md")
}

func migrationStateDir(root string) string {
	return filepath.Join(root, ".forge", "migrations")
}

func pendingPath(root string) string {
	return filepath.Join(migrationStateDir(root), "pending.v1.json")
}

func receiptPath(root string) string {
	path, _ := receiptPathForOperation(root, promotionOperationID)
	return path
}

func receiptPathForOperation(root, operation string) (string, error) {
	var name string
	switch operation {
	case promotionOperationID:
		name = lifecycleReceiptFile
	case manualModeOperationID:
		name = manualModeReceiptFile
	default:
		return "", fmt.Errorf("migrationtxn: unsupported receipt operation %q", operation)
	}
	return filepath.Join(migrationStateDir(root), name), nil
}

func inspectStateDir(root string) (bool, error) {
	for _, path := range []string{filepath.Join(root, ".forge"), migrationStateDir(root)} {
		_, present, err := statefs.InspectDir(path)
		if err != nil || !present {
			return false, err
		}
	}
	return true, nil
}

func readOptionalState(root, path string, ops fileOps) ([]byte, bool, error) {
	return readOptionalStateLimit(root, path, stateMaxBytes, ops)
}

func readOptionalStateLimit(
	root, path string,
	maxBytes int64,
	ops fileOps,
) ([]byte, bool, error) {
	present, err := inspectStateDir(root)
	if err != nil || !present {
		return nil, false, err
	}
	return ops.readState(path, maxBytes)
}

func ensureMigrationStateDir(root string, ops fileOps) error {
	for _, dir := range []string{
		filepath.Join(root, ".forge"),
		migrationStateDir(root),
	} {
		if err := ops.ensureStateDir(dir); err != nil {
			return fmt.Errorf("migrationtxn: secure state directory: %w", err)
		}
	}
	return nil
}
