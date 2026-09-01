package controlstore

import (
	"database/sql"
	"fmt"
)

func sqlOpen(dsn string) (*sql.DB, error) {
	db, err := sql.Open("sqlite", dsn)
	if err != nil {
		return nil, fmt.Errorf("create control database handle: %w", err)
	}
	return db, nil
}

// Close checkpoints and closes SQLite before releasing the bound directory.
func (store *Store) Close() error {
	if store == nil {
		return nil
	}
	store.closeOnce.Do(store.closeResources)
	return store.closeErr
}

func (store *Store) closeResources() {
	var dbErr, directoryErr error
	if store.db != nil {
		dbErr = store.db.Close()
	}
	if store.directory != nil {
		directoryErr = store.directory.Close()
	}
	if dbErr != nil {
		store.closeErr = dbErr
		return
	}
	store.closeErr = directoryErr
}
