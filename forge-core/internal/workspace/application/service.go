package application

import (
	"context"
	"fmt"
	"time"
)

const (
	maxCommitAttempts = 8
	maxListLimit      = 100
	maxListRead       = 1000
	maxListScan       = maxListRead - 1
	journalPageLimit  = 1
)

type identitySource func(string) (string, error)
type clockSource func() int64

// Service exposes internal Workspace Catalog v1 commands and reads.
type Service struct {
	journal    Journal
	identities identitySource
	now        clockSource
}

// NewService constructs an internal service with cryptographic event identities.
func NewService(journal Journal) (*Service, error) {
	return newService(journal, randomPlatformID, func() int64 {
		return time.Now().UnixMilli()
	})
}

func newService(journal Journal, identities identitySource, now clockSource) (*Service, error) {
	if journal == nil || identities == nil || now == nil {
		return nil, fmt.Errorf("workspace service requires journal, identity source and clock")
	}
	return &Service{journal: journal, identities: identities, now: now}, nil
}

func validateServiceContext(ctx context.Context, service *Service) error {
	if ctx == nil || service == nil || service.journal == nil {
		return fmt.Errorf("workspace operation requires a service and context")
	}
	return ctx.Err()
}
