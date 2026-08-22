// Package sessionworktree owns isolated Git worktrees and serialized local
// integration for Forge coding sessions.
package sessionworktree

import (
	"fmt"
	"path/filepath"
	"regexp"
	"time"
)

const (
	stateVersion  = 1
	maxStateBytes = 64 << 10
)

type Status string

const (
	StatusRunning     Status = "RUNNING"
	StatusReady       Status = "READY_TO_MERGE"
	StatusRebasing    Status = "REBASING"
	StatusValidating  Status = "VALIDATING"
	StatusMerging     Status = "MERGING"
	StatusMerged      Status = "MERGED"
	StatusCleaned     Status = "CLEANED"
	StatusConflict    Status = "CONFLICT"
	StatusTestFailed  Status = "TEST_FAILED"
	StatusMergeFailed Status = "MERGE_FAILED"
)

var sessionIDPattern = regexp.MustCompile(`^[a-z0-9][a-z0-9._-]{0,63}$`)

// Session is the durable control-plane record for one branch and worktree.
// It contains no prompt, source bytes, credentials, or validation output.
type Session struct {
	Version      int    `json:"version"`
	SessionID    string `json:"session_id"`
	Repository   string `json:"repository"`
	BaseBranch   string `json:"base_branch"`
	Branch       string `json:"branch"`
	Worktree     string `json:"worktree"`
	BaseCommit   string `json:"base_commit"`
	HeadCommit   string `json:"head_commit"`
	MergedCommit string `json:"merged_commit,omitempty"`
	Status       Status `json:"status"`
	Failure      string `json:"failure,omitempty"`
	CreatedAt    string `json:"created_at"`
	UpdatedAt    string `json:"updated_at"`
	QueuedAt     string `json:"queued_at,omitempty"`
}

func newSession(id string, repo repository, base, worktree, commit string, now time.Time) Session {
	stamp := now.UTC().Format(time.RFC3339Nano)
	return Session{
		Version: stateVersion, SessionID: id, Repository: repo.primary,
		BaseBranch: base, Branch: "session/" + id, Worktree: worktree,
		BaseCommit: commit, HeadCommit: commit, Status: StatusRunning,
		CreatedAt: stamp, UpdatedAt: stamp,
	}
}

func validateSessionID(id string) error {
	if !sessionIDPattern.MatchString(id) {
		return fmt.Errorf("session id must match %s", sessionIDPattern)
	}
	return nil
}

func (session Session) validate(repo repository) error {
	if session.Version != stateVersion || validateSessionID(session.SessionID) != nil {
		return fmt.Errorf("session state has an invalid version or id")
	}
	if session.Repository != repo.primary || !validAbsolute(session.Worktree) {
		return fmt.Errorf("session state repository or worktree is invalid")
	}
	if pathWithin(repo.primary, session.Worktree) || pathWithin(repo.commonDir, session.Worktree) {
		return fmt.Errorf("session worktree must remain outside repository control paths")
	}
	if session.Branch != "session/"+session.SessionID || session.BaseBranch == "" {
		return fmt.Errorf("session state branch binding is invalid")
	}
	if !isGitObjectID(session.BaseCommit) || !isGitObjectID(session.HeadCommit) {
		return fmt.Errorf("session state commit binding is invalid")
	}
	if session.MergedCommit != "" && !isGitObjectID(session.MergedCommit) {
		return fmt.Errorf("session state merged commit is invalid")
	}
	if !validStatus(session.Status) || !validTimestamp(session.CreatedAt) ||
		!validTimestamp(session.UpdatedAt) {
		return fmt.Errorf("session state lifecycle is invalid")
	}
	return nil
}

func validAbsolute(path string) bool {
	return filepath.IsAbs(path) && filepath.Clean(path) == path
}

func validTimestamp(value string) bool {
	_, err := time.Parse(time.RFC3339Nano, value)
	return err == nil
}

func validStatus(status Status) bool {
	switch status {
	case StatusRunning, StatusReady, StatusRebasing, StatusValidating,
		StatusMerging, StatusMerged, StatusCleaned, StatusConflict,
		StatusTestFailed, StatusMergeFailed:
		return true
	default:
		return false
	}
}

func isGitObjectID(value string) bool {
	if len(value) != 40 && len(value) != 64 {
		return false
	}
	for _, character := range value {
		if character < '0' || character > '9' {
			if character < 'a' || character > 'f' {
				return false
			}
		}
	}
	return true
}
