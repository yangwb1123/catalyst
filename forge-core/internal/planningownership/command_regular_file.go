package planningownership

import (
	"fmt"
	"os"
)

func readRegularFile(path string, maximum int) ([]byte, error) {
	return readRegularFileChecked(path, maximum, nil)
}

func readRegularFileChecked(path string, maximum int, afterRead func()) ([]byte, error) {
	file, err := openRegularNoFollow(path)
	if err != nil {
		return nil, err
	}
	openedBefore, err := file.Stat()
	pathBefore, pathErr := os.Lstat(path)
	if err != nil || pathErr != nil || !sameStableFile(openedBefore, pathBefore) {
		_ = file.Close()
		return nil, fmt.Errorf("input path is not a stable regular non-symlink file")
	}
	raw, readErr := readBounded(file, maximum)
	if afterRead != nil {
		afterRead()
	}
	openedAfter, statErr := file.Stat()
	pathAfter, pathErr := os.Lstat(path)
	closeErr := file.Close()
	if readErr != nil || statErr != nil || pathErr != nil || closeErr != nil ||
		!sameStableFile(openedBefore, openedAfter) || !sameStableFile(openedAfter, pathAfter) {
		return nil, fmt.Errorf("input file changed during bounded read")
	}
	return raw, nil
}

func sameStableFile(left, right os.FileInfo) bool {
	return left != nil && right != nil && left.Mode().IsRegular() && right.Mode().IsRegular() &&
		os.SameFile(left, right) && left.Mode() == right.Mode() && left.Size() == right.Size() &&
		left.ModTime().UnixNano() == right.ModTime().UnixNano() && samePlatformChangeTime(left, right)
}
