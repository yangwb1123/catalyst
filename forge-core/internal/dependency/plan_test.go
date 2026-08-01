package dependency

import (
	"strings"
	"testing"
)

func TestWavesPreservesAuthoredOrder(t *testing.T) {
	nodes := []Node{
		{ID: "root"},
		{ID: "right", Dependencies: []string{"root"}},
		{ID: "left", Dependencies: []string{"root"}},
		{ID: "join", Dependencies: []string{"left", "right"}},
	}
	got, err := Waves(nodes)
	if err != nil {
		t.Fatalf("Waves: %v", err)
	}
	assertWaves(t, got, [][]int{{0}, {1, 2}, {3}})
}

func TestWavesFanout(t *testing.T) {
	got, err := Waves([]Node{{ID: "a"}, {ID: "b"}, {ID: "c"}})
	if err != nil {
		t.Fatalf("Waves: %v", err)
	}
	assertWaves(t, got, [][]int{{0, 1, 2}})
}

func TestWavesFailsClosed(t *testing.T) {
	tests := []struct {
		name  string
		nodes []Node
		want  string
	}{
		{"empty identifier", []Node{{ID: ""}}, "empty identifier"},
		{"duplicate identifier", []Node{{ID: "a"}, {ID: "a"}}, "duplicates identifier"},
		{"unknown", []Node{{ID: "a", Dependencies: []string{"ghost"}}}, "unknown"},
		{"self", []Node{{ID: "a", Dependencies: []string{"a"}}}, "itself"},
		{"cycle", []Node{{ID: "a", Dependencies: []string{"b"}}, {ID: "b", Dependencies: []string{"a"}}}, "cycle"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			got, err := Waves(test.nodes)
			if err == nil || !strings.Contains(err.Error(), test.want) {
				t.Fatalf("Waves error = %v, want %q", err, test.want)
			}
			if got != nil {
				t.Fatalf("failed planning returned waves %v", got)
			}
		})
	}
}

func TestWavesAllowsRepeatedLegacyDependency(t *testing.T) {
	got, err := Waves([]Node{
		{ID: "a"},
		{ID: "b", Dependencies: []string{"a", "a"}},
	})
	if err != nil {
		t.Fatalf("Waves: %v", err)
	}
	assertWaves(t, got, [][]int{{0}, {1}})
}

func assertWaves(t *testing.T, got, want [][]int) {
	t.Helper()
	if len(got) != len(want) {
		t.Fatalf("wave count = %d, want %d: %v", len(got), len(want), got)
	}
	for index := range want {
		if !equalWave(got[index], want[index]) {
			t.Fatalf("wave[%d] = %v, want %v", index, got[index], want[index])
		}
	}
}

func equalWave(got, want []int) bool {
	if len(got) != len(want) {
		return false
	}
	for index := range want {
		if got[index] != want[index] {
			return false
		}
	}
	return true
}
