//go:build linux && !android

package controlstore

import (
	"context"
	"errors"
	"fmt"
	"io/fs"
	"net/url"
	"os"
	"strconv"

	_ "modernc.org/sqlite"
)

// OpenBound opens or initializes control.db relative to an already bound and
// locked App Server state root. The retained directory descriptor prevents a
// path replacement from separating the database from the instance lock.
func OpenBound(ctx context.Context, root *os.Root) (*Store, error) {
	if ctx == nil || root == nil {
		return nil, fmt.Errorf("control store requires a context and bound state root")
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	empty, err := ensureDatabaseFile(root)
	if err != nil {
		return nil, err
	}
	directory, err := root.Open(".")
	if err != nil {
		return nil, fmt.Errorf("open bound control directory: %w", err)
	}
	store, err := openDescriptorDatabase(ctx, directory, empty)
	if err != nil {
		_ = directory.Close()
		return nil, err
	}
	return store, nil
}

func ensureDatabaseFile(root *os.Root) (bool, error) {
	before, err := root.Lstat(databaseName)
	if errors.Is(err, fs.ErrNotExist) {
		return true, createDatabaseFile(root)
	}
	if err != nil {
		return false, fmt.Errorf("inspect %s: %w", databaseName, err)
	}
	return inspectDatabaseFile(root, before)
}

func inspectDatabaseFile(root *os.Root, before fs.FileInfo) (bool, error) {
	file, err := root.OpenFile(databaseName, os.O_RDWR, 0)
	if err != nil {
		return false, fmt.Errorf("open %s: %w", databaseName, err)
	}
	opened, statErr := file.Stat()
	headerErr := validateDatabaseHeader(file, opened)
	closeErr := file.Close()
	after, afterErr := root.Lstat(databaseName)
	if statErr != nil || closeErr != nil || afterErr != nil ||
		headerErr != nil ||
		validatePrivateDatabaseFile(opened) != nil ||
		!os.SameFile(before, opened) || !os.SameFile(opened, after) {
		if headerErr != nil {
			return false, headerErr
		}
		return false, fmt.Errorf("%s changed or is not private", databaseName)
	}
	return opened.Size() == 0, nil
}

func validateDatabaseHeader(file *os.File, info fs.FileInfo) error {
	if info == nil || info.Size() == 0 {
		return nil
	}
	if info.Size() < 100 {
		return fmt.Errorf("%w: truncated SQLite header", ErrSchemaIncompatible)
	}
	header := make([]byte, 100)
	if _, err := file.ReadAt(header, 0); err != nil {
		return fmt.Errorf("read control database header: %w", err)
	}
	if string(header[:16]) != "SQLite format 3\x00" {
		return fmt.Errorf("%w: foreign database header", ErrSchemaIncompatible)
	}
	return nil
}

func createDatabaseFile(root *os.Root) error {
	file, err := root.OpenFile(databaseName, os.O_RDWR|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		return fmt.Errorf("create %s: %w", databaseName, err)
	}
	if err = file.Chmod(0o600); err == nil {
		err = file.Sync()
	}
	info, statErr := file.Stat()
	closeErr := file.Close()
	after, afterErr := root.Lstat(databaseName)
	if err != nil || statErr != nil || closeErr != nil || afterErr != nil ||
		validatePrivateDatabaseFile(info) != nil || !os.SameFile(info, after) {
		return fmt.Errorf("initialize private %s", databaseName)
	}
	directory, err := root.Open(".")
	if err != nil {
		return fmt.Errorf("sync control directory: %w", err)
	}
	defer func() { _ = directory.Close() }()
	if err := directory.Sync(); err != nil {
		return fmt.Errorf("sync control directory: %w", err)
	}
	return nil
}

func openDescriptorDatabase(ctx context.Context, directory *os.File, empty bool) (*Store, error) {
	if !empty {
		if err := preflightDescriptorDatabase(ctx, directory); err != nil {
			return nil, err
		}
	}
	dsn := descriptorDSN(directory.Fd())
	db, err := sqlOpen(dsn)
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(1)
	db.SetMaxIdleConns(1)
	if err := db.PingContext(ctx); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("open bound control database: %w", err)
	}
	if err := migrateOrValidate(ctx, db); err != nil {
		_ = db.Close()
		return nil, err
	}
	return &Store{db: db, directory: directory}, nil
}

func preflightDescriptorDatabase(ctx context.Context, directory *os.File) error {
	db, err := sqlOpen(preflightDSN(directory.Fd()))
	if err != nil {
		return err
	}
	defer func() { _ = db.Close() }()
	db.SetMaxOpenConns(1)
	db.SetMaxIdleConns(1)
	if err := db.PingContext(ctx); err != nil {
		return fmt.Errorf("open read-only control database preflight: %w", err)
	}
	return validateExistingSchema(ctx, db)
}

func descriptorDSN(descriptor uintptr) string {
	path := descriptorDatabasePath(descriptor)
	query := url.Values{}
	query.Set("_defensive", "1")
	query.Set("_txlock", "immediate")
	query.Add("_pragma", "busy_timeout(5000)")
	query.Add("_pragma", "foreign_keys(1)")
	query.Add("_pragma", "journal_mode(WAL)")
	query.Add("_pragma", "recursive_triggers(0)")
	query.Add("_pragma", "synchronous(FULL)")
	query.Add("_pragma", "trusted_schema(0)")
	return (&url.URL{Scheme: "file", Path: path, RawQuery: query.Encode()}).String()
}

func preflightDSN(descriptor uintptr) string {
	query := url.Values{}
	query.Set("mode", "ro")
	return (&url.URL{Scheme: "file", Path: descriptorDatabasePath(descriptor), RawQuery: query.Encode()}).String()
}

func descriptorDatabasePath(descriptor uintptr) string {
	return "/proc/self/fd/" + strconv.FormatUint(uint64(descriptor), 10) + "/" + databaseName
}
