//go:build unix

package grantstate

import (
	"io/fs"
	"testing"
	"time"
)

type syntheticDirectoryInfo struct{ identity string }

func (i syntheticDirectoryInfo) Name() string     { return i.identity }
func (syntheticDirectoryInfo) Size() int64        { return 0 }
func (syntheticDirectoryInfo) Mode() fs.FileMode  { return fs.ModeDir | 0o700 }
func (syntheticDirectoryInfo) ModTime() time.Time { return time.Time{} }
func (syntheticDirectoryInfo) IsDir() bool        { return true }
func (syntheticDirectoryInfo) Sys() any           { return nil }

func TestRootOverlapUsesIdentityAcrossCaseAliases(t *testing.T) {
	probe := syntheticRootProbe(map[string]string{
		"/Volumes/Repo/.authority": "authority",
		"/Volumes/Repo":            "repository",
		"/Volumes":                 "volume",
		"/volumes/repo":            "repository",
		"/volumes":                 "volume",
		"/":                        "root",
	})
	overlap, err := rootsOverlapByIdentity(
		"/Volumes/Repo/.authority", syntheticDirectoryInfo{"authority"},
		"/volumes/repo", syntheticDirectoryInfo{"repository"}, probe,
	)
	if err != nil || !overlap {
		t.Fatalf("case-aliased repository ancestor = %v, %v", overlap, err)
	}
}

func TestRootOverlapUsesIdentityInReverseDirection(t *testing.T) {
	probe := syntheticRootProbe(map[string]string{
		"/Volumes/Auth":          "authority",
		"/Volumes":               "volume",
		"/volumes/auth/worktree": "repository",
		"/volumes/auth":          "authority",
		"/volumes":               "volume",
		"/":                      "root",
	})
	overlap, err := rootsOverlapByIdentity(
		"/Volumes/Auth", syntheticDirectoryInfo{"authority"},
		"/volumes/auth/worktree", syntheticDirectoryInfo{"repository"}, probe,
	)
	if err != nil || !overlap {
		t.Fatalf("case-aliased authority ancestor = %v, %v", overlap, err)
	}
}

func TestRootIdentityCheckAllowsDisjointSiblings(t *testing.T) {
	probe := syntheticRootProbe(map[string]string{
		"/Volume/authority":  "authority",
		"/Volume/repository": "repository",
		"/Volume":            "volume",
		"/":                  "root",
	})
	overlap, err := rootsOverlapByIdentity(
		"/Volume/authority", syntheticDirectoryInfo{"authority"},
		"/Volume/repository", syntheticDirectoryInfo{"repository"}, probe,
	)
	if err != nil || overlap {
		t.Fatalf("disjoint siblings = %v, %v", overlap, err)
	}
}

func syntheticRootProbe(identities map[string]string) rootIdentityProbe {
	return rootIdentityProbe{
		inspect: func(path string) (fs.FileInfo, error) {
			return syntheticDirectoryInfo{identities[path]}, nil
		},
		same: func(first, second fs.FileInfo) bool {
			return first.Name() != "" && first.Name() == second.Name()
		},
	}
}
