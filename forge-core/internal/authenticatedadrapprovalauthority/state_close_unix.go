//go:build unix

package authenticatedadrapprovalauthority

import "fmt"

func (s *unixState) close() error {
	var first error
	if s.lock != nil {
		if err := unlock(s.lock); err != nil {
			first = fmt.Errorf("unlock approval state: %w", err)
		}
		if err := s.lock.Close(); err != nil && first == nil {
			first = err
		}
		s.lock = nil
	}
	if s.state != nil {
		if err := s.state.Close(); err != nil && first == nil {
			first = err
		}
	}
	if s.stateDir != nil {
		if err := s.stateDir.Close(); err != nil && first == nil {
			first = err
		}
	}
	if s.authority != nil {
		if err := s.authority.Close(); err != nil && first == nil {
			first = err
		}
	}
	if s.authorityDir != nil {
		if err := s.authorityDir.Close(); err != nil && first == nil {
			first = err
		}
	}
	s.state, s.authority = nil, nil
	s.stateDir, s.authorityDir = nil, nil
	if s.repository != nil && s.repository.file != nil {
		if err := s.repository.file.Close(); err != nil && first == nil {
			first = err
		}
		s.repository.file = nil
	}
	return first
}
