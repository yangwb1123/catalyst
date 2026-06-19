package risk

import (
	"strings"
	"testing"
)

// classifyCase is one row of the Classify level-mapping table.
type classifyCase struct {
	name         string
	in           Signals
	wantLevel    string
	reasonHas    []string // substrings the reason must contain
	reasonHasNot []string // substrings it must NOT contain
}

// classifyCases is the level-mapping table — kept as a package var (not inside
// the test func) so the test body stays small and readable. It covers every band
// the prompt calls out: payment -> high, payment+irreversible -> critical,
// migration+prod -> high, ordinary -> low, and the empty value -> low.
var classifyCases = []classifyCase{
	{
		name:      "empty signals -> low",
		in:        Signals{},
		wantLevel: Low,
		reasonHas: []string{"low"},
	},
	{
		name:      "ordinary small reversible change -> low",
		in:        Signals{Reversible: true, BlastRadius: 1},
		wantLevel: Low,
		reasonHas: []string{"low"},
	},
	{
		name:      "payment alone -> high (security floor)",
		in:        Signals{TouchesPayment: true, Reversible: true},
		wantLevel: High,
		reasonHas: []string{"high", "payment"},
	},
	{
		name:      "auth alone -> high",
		in:        Signals{TouchesAuth: true, Reversible: true},
		wantLevel: High,
		reasonHas: []string{"high", "auth"},
	},
	{
		name:      "secrets alone -> high",
		in:        Signals{TouchesSecrets: true, Reversible: true},
		wantLevel: High,
		reasonHas: []string{"high", "secrets"},
	},
	{
		name:      "payment + irreversible -> critical",
		in:        Signals{TouchesPayment: true, Reversible: false},
		wantLevel: Critical,
		reasonHas: []string{"critical", "irreversible", "payment"},
	},
	{
		name:      "irreversible + large blast radius -> critical",
		in:        Signals{Reversible: false, BlastRadius: 8},
		wantLevel: Critical,
		reasonHas: []string{"critical", "irreversible", "blast"},
	},
	{
		name:      "irreversible + production traffic -> critical",
		in:        Signals{Reversible: false, ProdTraffic: true},
		wantLevel: Critical,
		reasonHas: []string{"critical", "irreversible", "production"},
	},
	{
		name:      "migration + prod traffic (reversible) -> high",
		in:        Signals{TouchesMigration: true, ProdTraffic: true, Reversible: true},
		wantLevel: High,
		reasonHas: []string{"high", "migration"},
	},
	{
		name:      "migration + prod traffic + irreversible -> critical",
		in:        Signals{TouchesMigration: true, ProdTraffic: true, Reversible: false},
		wantLevel: Critical,
		reasonHas: []string{"critical", "irreversible"},
	},
	{
		name:      "reversible migration alone -> medium",
		in:        Signals{TouchesMigration: true, Reversible: true},
		wantLevel: Medium,
		reasonHas: []string{"medium", "migration"},
	},
	{
		name:      "production traffic alone (reversible) -> medium",
		in:        Signals{ProdTraffic: true, Reversible: true},
		wantLevel: Medium,
		reasonHas: []string{"medium", "production"},
	},
	{
		name:      "non-trivial blast radius (reversible) -> medium",
		in:        Signals{BlastRadius: 3, Reversible: true},
		wantLevel: Medium,
		reasonHas: []string{"medium", "blast"},
	},
	{
		name:         "payment beats blast-radius: high not medium when reversible",
		in:           Signals{TouchesPayment: true, BlastRadius: 3, Reversible: true},
		wantLevel:    High,
		reasonHas:    []string{"high", "payment"},
		reasonHasNot: []string{"medium"},
	},
}

// TestClassify_Levels runs the classifyCases table through Classify, asserting
// both the level and that the reason names the deciding factors.
func TestClassify_Levels(t *testing.T) {
	for _, c := range classifyCases {
		t.Run(c.name, func(t *testing.T) {
			gotLevel, gotReason := Classify(c.in)
			if gotLevel != c.wantLevel {
				t.Errorf("Classify(%+v) level = %q, want %q (reason %q)", c.in, gotLevel, c.wantLevel, gotReason)
			}
			for _, sub := range c.reasonHas {
				if !strings.Contains(gotReason, sub) {
					t.Errorf("Classify(%+v) reason = %q, want substring %q", c.in, gotReason, sub)
				}
			}
			for _, sub := range c.reasonHasNot {
				if strings.Contains(gotReason, sub) {
					t.Errorf("Classify(%+v) reason = %q, must NOT contain %q", c.in, gotReason, sub)
				}
			}
		})
	}
}

// The reason must lead with the level token (e.g. "critical: ..."), so a report
// line can be split on ": " honestly.
func TestClassify_ReasonLeadsWithLevel(t *testing.T) {
	level, reason := Classify(Signals{TouchesPayment: true, Reversible: false})
	if !strings.HasPrefix(reason, level+": ") {
		t.Errorf("reason %q must start with %q", reason, level+": ")
	}
}

// Rank / Higher: severity ordering and the "raise, never lower" combine used by
// the manual-override merge in route.go.
func TestRankAndHigher(t *testing.T) {
	if !(Rank(Low) < Rank(Medium) && Rank(Medium) < Rank(High) && Rank(High) < Rank(Critical)) {
		t.Fatalf("Rank ordering broken: low=%d medium=%d high=%d critical=%d",
			Rank(Low), Rank(Medium), Rank(High), Rank(Critical))
	}
	if Rank("nonsense") != 0 {
		t.Errorf("unknown level Rank = %d, want 0 (low)", Rank("nonsense"))
	}
	if got := Higher(High, Low); got != High {
		t.Errorf("Higher(high, low) = %q, want high", got)
	}
	if got := Higher(Medium, Critical); got != Critical {
		t.Errorf("Higher(medium, critical) = %q, want critical", got)
	}
	if got := Higher(High, High); got != High {
		t.Errorf("Higher(high, high) = %q, want high (tie -> a)", got)
	}
}
