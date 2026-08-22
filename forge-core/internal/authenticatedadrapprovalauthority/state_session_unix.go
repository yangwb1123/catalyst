//go:build unix

package authenticatedadrapprovalauthority

import (
	"fmt"
	"io/fs"
)

func (s *protectedSession) current() (stateSnapshot, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.backend == nil {
		return stateSnapshot{}, fmt.Errorf("approval state session is closed")
	}
	value, err := s.backend.current()
	value.Data = cloneBytes(value.Data)
	return value, err
}

func (s *protectedSession) commit(expected stateSnapshot, next []byte) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.backend == nil {
		return fmt.Errorf("approval state session is closed")
	}
	expected.Data = cloneBytes(expected.Data)
	return s.backend.commit(expected, cloneBytes(next))
}

func (s *protectedSession) readLeaf(relative string, maximum int64,
	mode fs.FileMode) ([]byte, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.backend == nil {
		return nil, fmt.Errorf("approval state session is closed")
	}
	value, err := s.backend.readLeaf(relative, maximum, mode)
	if err != nil {
		return discardBytes(value), err
	}
	result := cloneBytes(value)
	clearBytes(value)
	return result, nil
}

func (s *protectedSession) close() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.backend == nil {
		return nil
	}
	err := s.backend.close()
	s.backend = nil
	return err
}
