package evolvescan

import (
	"strings"
	"testing"
)

func TestInstructionsCarryDepthSpecificContract(t *testing.T) {
	tests := []struct {
		depth string
		want  []string
	}{
		{DepthOpportunistic, []string{"obvious=true", "Do not claim full-dimensional"}},
		{DepthThorough, []string{"code, dependencies, security, performance, architecture_drift, test_coverage", "candidate_task"}},
		{DepthStandard, []string{"standard scan", "without claiming full-dimensional"}},
		{DepthAdvisory, []string{"advisory scan", "do not imply implementation authority"}},
	}
	for _, tc := range tests {
		t.Run(tc.depth, func(t *testing.T) {
			got, err := Instructions(tc.depth)
			if err != nil {
				t.Fatal(err)
			}
			for _, want := range append(tc.want, MarkerPrefix, `"depth":"`+tc.depth+`"`) {
				if !strings.Contains(got, want) {
					t.Errorf("Instructions(%q) lacks %q:\n%s", tc.depth, want, got)
				}
			}
		})
	}
}

func TestInstructionsRejectUnknownDepth(t *testing.T) {
	if _, err := Instructions("deep-ish"); err == nil {
		t.Fatal("unknown depth must fail")
	}
}

func TestDimensionsReturnsDefensiveCopy(t *testing.T) {
	got := Dimensions()
	got[0] = "mutated"
	if Dimensions()[0] != "code" {
		t.Fatal("Dimensions exposed shared storage")
	}
}
