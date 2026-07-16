package orchestrator

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/converge"
)

// loop_honesty_test.go — split out of loop_test.go (which was pressed against the
// 500-line volume gate) to keep both files comfortably under it. This file owns
// reportConvergence's honesty-warning tests (today: the FileDelta cross-check).

// reportConvergence's FileDelta honesty warning ("agent self-report may overstate
// progress") must be CONDITIONAL on the real signal, not a permanent false
// positive. Before Signals.FileDelta was wired (computeFileDelta, cmd/forge/gates.go),
// it was NEVER assigned and therefore always exactly 0.0 — which is always < 0.3,
// so this warning fired on EVERY iteration with roadmap_completion > 50%, regardless
// of whether files actually changed. This test proves the warning now genuinely
// tracks FileDelta: absent when file evidence is real (>= 0.3), present when it is
// not — driven directly via reportConvergence (no full Run needed) against a
// captured Log.
func TestReportConvergence_FileDeltaWarningTracksRealSignal(t *testing.T) {
	const honestyMarker = "honesty: roadmap="

	captureLog := func(sig converge.Signals) []string {
		var logs []string
		l := LoopEngine{
			Stop: conjunctionStop(roadmapDone()),
			Log:  func(s string) { logs = append(logs, s) },
		}
		l.reportConvergence(sig)
		return logs
	}

	// High roadmap completion + LOW FileDelta (little/no file evidence) -> the
	// warning must fire — this is the genuine "self-report may overstate progress"
	// case the warning exists to catch.
	lowEvidence := captureLog(converge.Signals{RoadmapCompletion: 0.9, FileDelta: 0.1})
	if !containsSubstring(lowEvidence, honestyMarker) {
		t.Errorf("roadmap=90%%, FileDelta=10%% must trip the honesty warning; got logs %v", lowEvidence)
	}

	// High roadmap completion + HIGH FileDelta (real file evidence) -> the warning
	// must be ABSENT. Before the fix this was IMPOSSIBLE to observe: FileDelta was
	// always 0.0, so this exact case (real progress, real files touched) would have
	// still spuriously fired.
	realEvidence := captureLog(converge.Signals{RoadmapCompletion: 0.9, FileDelta: 0.8})
	if containsSubstring(realEvidence, honestyMarker) {
		t.Errorf("roadmap=90%%, FileDelta=80%% (real file evidence) must NOT trip the honesty warning; got logs %v", realEvidence)
	}

	// Low roadmap completion (<=50%) never trips the warning regardless of FileDelta
	// — the warning only ever applies once real completion is claimed.
	lowRoadmap := captureLog(converge.Signals{RoadmapCompletion: 0.2, FileDelta: 0.0})
	if containsSubstring(lowRoadmap, honestyMarker) {
		t.Errorf("roadmap=20%% must never trip the honesty warning regardless of FileDelta; got logs %v", lowRoadmap)
	}
}

// containsSubstring reports whether any log line in lines contains sub.
func containsSubstring(lines []string, sub string) bool {
	for _, l := range lines {
		if strings.Contains(l, sub) {
			return true
		}
	}
	return false
}
