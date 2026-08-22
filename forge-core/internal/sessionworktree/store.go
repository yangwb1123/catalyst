package sessionworktree

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"forgeos/forge-core/internal/statefs"
)

type sessionStore struct {
	repo repository
	dir  string
}

func openStore(repo repository) (*sessionStore, error) {
	root := filepath.Join(repo.commonDir, "forge-sessions")
	dir := filepath.Join(root, "sessions")
	if err := statefs.EnsurePrivateDirTree(dir); err != nil {
		return nil, fmt.Errorf("secure session state: %w", err)
	}
	return &sessionStore{repo: repo, dir: dir}, nil
}

func (store *sessionStore) path(id string) (string, error) {
	if err := validateSessionID(id); err != nil {
		return "", err
	}
	return filepath.Join(store.dir, id+".json"), nil
}

func (store *sessionStore) create(session Session) error {
	path, err := store.path(session.SessionID)
	if err != nil {
		return err
	}
	if _, present, err := statefs.InspectRegular(path); err != nil || present {
		if err == nil {
			err = fmt.Errorf("session %q already exists", session.SessionID)
		}
		return err
	}
	return store.save(session)
}

func (store *sessionStore) save(session Session) error {
	if err := session.validate(store.repo); err != nil {
		return err
	}
	encoded, err := json.Marshal(session)
	if err != nil {
		return fmt.Errorf("encode session state: %w", err)
	}
	path, err := store.path(session.SessionID)
	if err != nil {
		return err
	}
	return statefs.AtomicWrite(path, append(encoded, '\n'), 0o600)
}

func (store *sessionStore) load(id string) (Session, error) {
	path, err := store.path(id)
	if err != nil {
		return Session{}, err
	}
	raw, present, err := statefs.ReadRegularUnmodified(path, maxStateBytes)
	if err != nil || !present {
		if err == nil {
			err = fmt.Errorf("session %q does not exist", id)
		}
		return Session{}, err
	}
	return decodeSession(raw, store.repo)
}

func decodeSession(raw []byte, repo repository) (Session, error) {
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	var session Session
	if err := decoder.Decode(&session); err != nil {
		return Session{}, fmt.Errorf("decode session state: %w", err)
	}
	if err := requireJSONEOF(decoder); err != nil {
		return Session{}, err
	}
	if err := session.validate(repo); err != nil {
		return Session{}, err
	}
	return session, nil
}

func requireJSONEOF(decoder *json.Decoder) error {
	var extra any
	if err := decoder.Decode(&extra); err != io.EOF {
		return fmt.Errorf("session state has trailing JSON")
	}
	return nil
}

func (store *sessionStore) list() ([]Session, error) {
	entries, err := os.ReadDir(store.dir)
	if err != nil {
		return nil, fmt.Errorf("list session state: %w", err)
	}
	sessions := make([]Session, 0, len(entries))
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".json") {
			return nil, fmt.Errorf("unexpected session state entry %q", entry.Name())
		}
		session, err := store.load(strings.TrimSuffix(entry.Name(), ".json"))
		if err != nil {
			return nil, err
		}
		sessions = append(sessions, session)
	}
	sort.Slice(sessions, func(left, right int) bool {
		return sessions[left].SessionID < sessions[right].SessionID
	})
	return sessions, nil
}

func readySessions(sessions []Session) []Session {
	ready := make([]Session, 0, len(sessions))
	for _, session := range sessions {
		if session.Status == StatusReady {
			ready = append(ready, session)
		}
	}
	sort.Slice(ready, func(left, right int) bool {
		if ready[left].QueuedAt == ready[right].QueuedAt {
			return ready[left].SessionID < ready[right].SessionID
		}
		return ready[left].QueuedAt < ready[right].QueuedAt
	})
	return ready
}

func transition(session *Session, status Status, failure string, now time.Time) {
	session.Status = status
	session.Failure = failure
	session.UpdatedAt = now.UTC().Format(time.RFC3339Nano)
}
