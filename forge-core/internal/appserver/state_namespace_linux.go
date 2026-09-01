//go:build linux

package appserver

import (
	"bufio"
	"bytes"
	"io"
	"os"
	"strconv"
	"strings"
)

func trustedUnmappedOwner(owner int) bool {
	overflow, present := configuredOverflowUID()
	if !present || owner != overflow {
		return false
	}
	file, err := os.Open("/proc/self/uid_map")
	if err != nil {
		return false
	}
	defer func() { _ = file.Close() }()
	return uidMapTrustsUnmappedOwner(owner, file)
}

func uidMapTrustsUnmappedOwner(owner int, source io.Reader) bool {
	data, err := io.ReadAll(io.LimitReader(source, 4097))
	if err != nil || len(data) > 4096 {
		return false
	}
	scanner := bufio.NewScanner(bytes.NewReader(data))
	scanner.Buffer(make([]byte, 256), 4096)
	lines := 0
	initial := false
	mappedOwner := false
	for scanner.Scan() {
		lines++
		fields := strings.Fields(scanner.Text())
		if len(fields) != 3 {
			return false
		}
		inside, insideErr := strconv.ParseUint(fields[0], 10, 32)
		outside, outsideErr := strconv.ParseUint(fields[1], 10, 32)
		length, lengthErr := strconv.ParseUint(fields[2], 10, 32)
		if insideErr != nil || outsideErr != nil || lengthErr != nil {
			return false
		}
		if length == 0 {
			return false
		}
		ownerValue := uint64(owner)
		if ownerValue >= inside && ownerValue-inside < length {
			mappedOwner = true
		}
		initial = inside == 0 && outside == 0 && length == 4_294_967_295
	}
	return scanner.Err() == nil && lines > 0 && !(lines == 1 && initial) && !mappedOwner
}

func configuredOverflowUID() (int, bool) {
	file, err := os.Open("/proc/sys/kernel/overflowuid")
	if err != nil {
		return 0, false
	}
	defer func() { _ = file.Close() }()
	return parseOverflowUID(file)
}

func parseOverflowUID(source io.Reader) (int, bool) {
	data, err := io.ReadAll(io.LimitReader(source, 33))
	if err != nil || len(data) == 0 || len(data) > 32 {
		return 0, false
	}
	value, err := strconv.ParseUint(strings.TrimSpace(string(data)), 10, 32)
	if err != nil || value > uint64(^uint(0)>>1) {
		return 0, false
	}
	return int(value), true
}
