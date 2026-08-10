package gitworktreesource

// RegularReadLimits bounds one source-manifest-bound regular-file read.
// Limits must be positive and cannot exceed the parent source profile.
type RegularReadLimits struct {
	MaxFiles      int
	MaxFileBytes  int64
	MaxTotalBytes int64
	MaxPathDepth  int
}

// RegularFile is one exact regular file whose bytes and digest match the
// corresponding entry in a previously captured source manifest.
type RegularFile struct {
	Content []byte
	Path    string
	SHA256  string
}

func cloneRegularFiles(values []RegularFile) []RegularFile {
	result := make([]RegularFile, len(values))
	for index, value := range values {
		value.Content = append([]byte(nil), value.Content...)
		result[index] = value
	}
	return result
}
