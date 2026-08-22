//go:build unix

package grantstate

import "fmt"

func (s *unixSession) close() error {
	var first error
	if s.lock != nil {
		if err := unlock(s.lock); err != nil {
			first = fmt.Errorf("unlock stable lock: %w", err)
		}
		if err := s.lock.Close(); err != nil && first == nil {
			first = fmt.Errorf("close stable lock: %w", err)
		}
		s.lock = nil
	}
	if s.state != nil {
		if err := s.state.Close(); err != nil && first == nil {
			first = fmt.Errorf("close state directory: %w", err)
		}
		s.state = nil
	}
	if s.stateDirFile != nil {
		if err := s.stateDirFile.Close(); err != nil && first == nil {
			first = fmt.Errorf("close state directory file: %w", err)
		}
		s.stateDirFile = nil
	}
	if s.authority != nil {
		if err := s.authority.Close(); err != nil && first == nil {
			first = fmt.Errorf("close authority root: %w", err)
		}
		s.authority = nil
	}
	if s.authorityDir != nil {
		if err := s.authorityDir.Close(); err != nil && first == nil {
			first = fmt.Errorf("close authority directory file: %w", err)
		}
		s.authorityDir = nil
	}
	if s.repositoryDir != nil {
		if err := s.repositoryDir.Close(); err != nil && first == nil {
			first = fmt.Errorf("close repository directory file: %w", err)
		}
		s.repositoryDir = nil
		s.clearRepository()
	}
	if first != nil {
		return newError(CodeUnsafe, "close", "release protected state", first)
	}
	return nil
}
