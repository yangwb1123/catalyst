package gopackagedependencyobservationproducer

import (
	"forgeos/forge-core/internal/gitworktreesource"
	"forgeos/forge-core/internal/gopackagegraph"
)

func graphSourceEntries(manifest gitworktreesource.SourceManifest) []gopackagegraph.SourceEntry {
	result := make([]gopackagegraph.SourceEntry, len(manifest.Entries))
	for index, entry := range manifest.Entries {
		result[index] = gopackagegraph.SourceEntry{
			Bytes: entry.Bytes, ContentSHA256: cloneString(entry.ContentSHA256),
			Kind: entry.Kind, Path: entry.Path,
		}
	}
	return result
}

func graphRegularFile(value gitworktreesource.RegularFile) gopackagegraph.RegularFile {
	return gopackagegraph.RegularFile{
		Content: append([]byte(nil), value.Content...), Path: value.Path, SHA256: value.SHA256,
	}
}

func graphRegularFiles(values []gitworktreesource.RegularFile) []gopackagegraph.RegularFile {
	result := make([]gopackagegraph.RegularFile, len(values))
	for index, value := range values {
		result[index] = graphRegularFile(value)
	}
	return result
}

func cloneString(value *string) *string {
	if value == nil {
		return nil
	}
	result := *value
	return &result
}
