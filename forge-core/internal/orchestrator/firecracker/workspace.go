package firecracker

import (
	"context"
	"fmt"
	"io"
	"os"
	"path/filepath"
)

const copyBufferBytes = 32 * 1024

// copyTree copies a rootfs template without following symlinks. Directories,
// regular files, and symlinks are the complete accepted entry set; devices,
// sockets, and pipes fail closed instead of being opened or copied.
func copyTree(ctx context.Context, from, to string) error {
	root, err := os.Lstat(from)
	if err != nil {
		return err
	}
	if !root.IsDir() {
		return fmt.Errorf("rootfs template root is not a directory: %s", root.Mode())
	}
	return filepath.Walk(from, func(path string, info os.FileInfo, walkErr error) error {
		if err := ctx.Err(); err != nil {
			return err
		}
		if walkErr != nil {
			return walkErr
		}
		rel, err := filepath.Rel(from, path)
		if err != nil {
			return err
		}
		target := filepath.Join(to, rel)
		switch {
		case info.IsDir():
			return os.MkdirAll(target, info.Mode().Perm())
		case info.Mode()&os.ModeSymlink != 0:
			link, err := os.Readlink(path)
			if err != nil {
				return err
			}
			return os.Symlink(link, target)
		case info.Mode().IsRegular():
			return copyRegularFile(ctx, path, target, info.Mode().Perm())
		default:
			return fmt.Errorf("unsupported rootfs template entry %q (%s)", rel, info.Mode())
		}
	})
}

// copyRegularFile keeps memory bounded and observes cancellation between
// chunks. O_EXCL also prevents an unexpected target entry from being followed.
func copyRegularFile(ctx context.Context, from, to string, mode os.FileMode) (returnErr error) {
	source, err := os.Open(from)
	if err != nil {
		return err
	}
	defer func() {
		if err := source.Close(); returnErr == nil && err != nil {
			returnErr = err
		}
	}()
	target, err := os.OpenFile(to, os.O_WRONLY|os.O_CREATE|os.O_EXCL, mode)
	if err != nil {
		return err
	}
	complete := false
	defer func() {
		if err := target.Close(); returnErr == nil && err != nil {
			returnErr = err
		}
		if !complete {
			_ = os.Remove(to)
		}
	}()
	buffer := make([]byte, copyBufferBytes)
	for {
		if err := ctx.Err(); err != nil {
			return err
		}
		count, readErr := source.Read(buffer)
		if count > 0 {
			written, err := target.Write(buffer[:count])
			if err != nil {
				return err
			}
			if written != count {
				return io.ErrShortWrite
			}
		}
		if readErr == io.EOF {
			complete = true
			return nil
		}
		if readErr != nil {
			return readErr
		}
	}
}

// writeInjectedFile replaces only a copied regular entry, never a symlink or
// special file. The final create is exclusive so the injected guest input
// cannot escape the temporary root through a template-controlled link.
func writeInjectedFile(path string, content []byte, mode os.FileMode) error {
	info, err := os.Lstat(path)
	if err == nil {
		if !info.Mode().IsRegular() {
			return fmt.Errorf("injection target is not a regular file: %s", info.Mode())
		}
		if err := os.Remove(path); err != nil {
			return err
		}
	} else if !os.IsNotExist(err) {
		return err
	}
	file, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_EXCL, mode)
	if err != nil {
		return err
	}
	written, err := file.Write(content)
	if err != nil || written != len(content) {
		_ = file.Close()
		_ = os.Remove(path)
		if err != nil {
			return err
		}
		return io.ErrShortWrite
	}
	return file.Close()
}
