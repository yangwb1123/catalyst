package doctor

import (
	"fmt"
	"os"
	"path/filepath"
	"time"

	"forgeos/forge-core/internal/persist"
)

// FileStat is a repo-state snapshot of one .forge/ file. Size is the raw
// byte count (for JSON's size_bytes); SizeText/Age are pre-rendered for text
// display ("N.N KB" / "today"); Age is "" when Exists is true but the OS
// reports a zero mtime (extremely rare) — mirroring the original
// statFileMeta's "leave Age unset" case. All fields but Exists/Path are
// zero-valued when Exists is false.
type FileStat struct {
	Exists   bool
	Path     string
	Size     int64
	SizeText string
	Age      string
}

// CheckpointInfo summarizes checkpoint.json's content for `forge status`.
// Found is persist.Load's raw "found" (true only once decode succeeds);
// ParseOK is kept as a distinct field (rather than assumed equal to Found)
// so this struct stays a faithful mirror of the two-value legacy check even
// if persist.Load's found/error contract ever changes.
type CheckpointInfo struct {
	Found             bool
	ParseOK           bool
	Iteration         int
	RoadmapCompletion float64
	GatesGreen        bool
	Mode              string
}

// StatusSnapshot is the full repo-state result of Status. DotForgeMissing
// short-circuits everything else: when true, .forge/ does not exist and no
// runs have been executed yet.
type StatusSnapshot struct {
	DotForge          string
	DotForgeMissing   bool
	Checkpoint        FileStat
	CheckpointInfo    CheckpointInfo
	CheckpointHistory int
	Trace             FileStat
	TraceBackup       FileStat
	Memory            FileStat
}

// Status inspects <root>/.forge and returns a full snapshot: file stats for
// checkpoint.json / trace.jsonl / trace.jsonl.1 / memory.jsonl, the
// checkpoint history backup count, and the parsed checkpoint content.
func Status(root string) StatusSnapshot {
	dotForge := dotForgeDir(root)
	snap := StatusSnapshot{DotForge: dotForge}
	if _, err := os.Stat(dotForge); os.IsNotExist(err) {
		snap.DotForgeMissing = true
		return snap
	}
	snap.Checkpoint = statFile(filepath.Join(dotForge, "checkpoint.json"))
	snap.CheckpointHistory = CheckpointHistoryCount(dotForge)
	snap.Trace = statFile(filepath.Join(dotForge, "trace.jsonl"))
	snap.TraceBackup = statFile(filepath.Join(dotForge, "trace.jsonl.1"))
	snap.Memory = statFile(filepath.Join(dotForge, "memory.jsonl"))
	snap.CheckpointInfo = loadCheckpointInfo(dotForge)
	return snap
}

// statFile stats path and reports its size/age; a non-existent path returns
// a zero FileStat with Exists=false.
func statFile(path string) FileStat {
	st, err := os.Stat(path)
	if os.IsNotExist(err) {
		return FileStat{Path: path}
	}
	return FileStat{
		Exists: true, Path: path, Size: st.Size(),
		SizeText: sizeText(st.Size()), Age: ageString(st.ModTime()),
	}
}

// loadCheckpointInfo loads and summarizes checkpoint.json; ParseOK is true
// only when the load found AND cleanly decoded the file.
func loadCheckpointInfo(dotForge string) CheckpointInfo {
	cpPath := filepath.Join(dotForge, "checkpoint.json")
	cp, found, err := persist.Load(cpPath)
	info := CheckpointInfo{Found: found}
	if err == nil && found {
		info.ParseOK = true
		info.Iteration = cp.Iteration
		info.RoadmapCompletion = cp.RoadmapCompletion
		info.GatesGreen = cp.GatesGreen
		info.Mode = cp.Mode
	}
	return info
}

// CheckpointHistoryCount returns the number of checkpoint history backup
// files (checkpoint.json.1 through .5, retain=5) present under dotForge.
func CheckpointHistoryCount(dotForge string) int {
	cpPath := filepath.Join(dotForge, "checkpoint.json")
	count := 0
	for i := 1; i <= 5; i++ {
		if _, err := os.Stat(fmt.Sprintf("%s.%d", cpPath, i)); err == nil {
			count++
		}
	}
	return count
}

// HistoryLines renders `forge status --history`'s checkpoint chain as
// pre-formatted table rows (label, iteration, roadmap%, gates, age, mode) —
// everything but the header, which the CLI layer owns. An empty result means
// no checkpoint history was found.
func HistoryLines(root string) []string {
	chain := LoadCheckpointChain(root)
	lines := make([]string, len(chain))
	for i, cp := range chain {
		age := ""
		if cp.UpdatedAtUnix > 0 {
			if days := int(time.Now().Unix()-cp.UpdatedAtUnix) / 86400; days > 0 {
				age = fmt.Sprintf("%dd", days)
			} else {
				age = "today"
			}
		}
		label := "current"
		if i > 0 {
			label = fmt.Sprintf(".%d", i)
		}
		lines[i] = fmt.Sprintf("  %-8s %-6d %-8.0f %-6v %-10s %s",
			label, cp.Iteration, cp.RoadmapCompletion*100, cp.GatesGreen, age, cp.Mode)
	}
	return lines
}
