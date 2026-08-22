package projectsnapshot

import (
	"bytes"
	"fmt"
	"regexp"
)

var indexDebugBlock = regexp.MustCompile(
	`^  ctime: (0|[1-9][0-9]*):(0|[1-9][0-9]*)\n` +
		`  mtime: (0|[1-9][0-9]*):(0|[1-9][0-9]*)\n` +
		`  dev: (0|[1-9][0-9]*)\tino: (0|[1-9][0-9]*)\n` +
		`  uid: (0|[1-9][0-9]*)\tgid: (0|[1-9][0-9]*)\n` +
		`  size: (0|[1-9][0-9]*)\tflags: (0|[1-9a-f][0-9a-f]*)\n`,
)

func validateTrackedIndexDebug(raw []byte, tracked map[string]struct{}) error {
	seen := make(map[string]struct{}, len(tracked))
	for offset := 0; offset < len(raw); {
		end := bytes.IndexByte(raw[offset:], 0)
		if end < 0 {
			return fmt.Errorf("tracked index debug path is not NUL terminated")
		}
		path := string(raw[offset : offset+end])
		if err := validateIndexDebugPath(path, tracked, seen); err != nil {
			return err
		}
		block := raw[offset+end+1:]
		match := indexDebugBlock.FindSubmatchIndex(block)
		if match == nil || match[0] != 0 {
			return fmt.Errorf("tracked index debug record is malformed")
		}
		flags := string(block[match[len(match)-2]:match[len(match)-1]])
		if flags != "0" {
			return fmt.Errorf("tracked path has unsupported index flags")
		}
		offset += end + 1 + match[1]
	}
	if len(seen) != len(tracked) {
		return fmt.Errorf("tracked index debug set is incomplete")
	}
	return nil
}

func validateIndexDebugPath(
	path string,
	tracked map[string]struct{},
	seen map[string]struct{},
) error {
	if err := validateInventoryPath(path); err != nil {
		return err
	}
	if _, exists := tracked[path]; !exists {
		return fmt.Errorf("tracked index debug path is outside the stage-zero set")
	}
	if _, duplicate := seen[path]; duplicate {
		return fmt.Errorf("tracked index debug path is duplicated")
	}
	seen[path] = struct{}{}
	return nil
}
