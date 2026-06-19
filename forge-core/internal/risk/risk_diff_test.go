package risk

import (
	"strings"
	"testing"
)

// fromPathsCase is one row of the FromChangedPaths heuristic table.
type fromPathsCase struct {
	name        string
	paths       []string
	want        Signals
	reasonHas   []string // substrings the JOINED reasons must contain
	reasonEmpty bool     // reasons must be empty (no surface matched)
}

// fromPathsCases is the path-substring mapping table — kept as a package var so
// the test body stays small. It covers every needle group the prompt calls out
// plus BlastRadius=n, the empty input (zero Signals), and case-insensitivity.
var fromPathsCases = []fromPathsCase{
	{
		name:        "empty input -> zero Signals, no reasons",
		paths:       nil,
		want:        Signals{},
		reasonEmpty: true,
	},
	{
		name:      "payment path -> TouchesPayment, reversible, blast=1",
		paths:     []string{"src/payment/charge.go"},
		want:      Signals{TouchesPayment: true, Reversible: true, BlastRadius: 1},
		reasonHas: []string{"payment"},
	},
	{
		name:      "billing/charge/invoice also map to payment",
		paths:     []string{"internal/billing/invoice.go"},
		want:      Signals{TouchesPayment: true, Reversible: true, BlastRadius: 1},
		reasonHas: []string{"payment"},
	},
	{
		name:      "auth path -> TouchesAuth",
		paths:     []string{"pkg/login/session.go"},
		want:      Signals{TouchesAuth: true, Reversible: true, BlastRadius: 1},
		reasonHas: []string{"auth"},
	},
	{
		name:      "rbac/permission also map to auth",
		paths:     []string{"internal/rbac/permission.go"},
		want:      Signals{TouchesAuth: true, Reversible: true, BlastRadius: 1},
		reasonHas: []string{"auth"},
	},
	{
		name:      "secret path -> TouchesSecrets",
		paths:     []string{"config/credentials.go"},
		want:      Signals{TouchesSecrets: true, Reversible: true, BlastRadius: 1},
		reasonHas: []string{"secrets"},
	},
	{
		name:      ".pem / .key extensions map to secrets",
		paths:     []string{"deploy/tls/server.pem"},
		want:      Signals{TouchesSecrets: true, Reversible: true, BlastRadius: 1},
		reasonHas: []string{"secrets"},
	},
	{
		name:      "migration path -> TouchesMigration AND Reversible=false",
		paths:     []string{"db/migrations/001_init.sql"},
		want:      Signals{TouchesMigration: true, Reversible: false, BlastRadius: 1},
		reasonHas: []string{"migration", "irreversible"},
	},
	{
		name:      "schema / .sql also map to migration (irreversible)",
		paths:     []string{"internal/store/schema.go"},
		want:      Signals{TouchesMigration: true, Reversible: false, BlastRadius: 1},
		reasonHas: []string{"migration", "irreversible"},
	},
	{
		name:  "multiple files -> BlastRadius = n, multiple surfaces",
		paths: []string{"src/payment/charge.go", "db/migrations/001.sql", "pkg/util/x.go"},
		want: Signals{
			TouchesPayment:   true,
			TouchesMigration: true,
			Reversible:       false, // migration forces it down
			BlastRadius:      3,
		},
		reasonHas: []string{"payment", "migration"},
	},
	{
		name:        "ordinary paths -> no surface, reversible, blast=n, no reasons",
		paths:       []string{"src/widget/render.go", "docs/readme.md"},
		want:        Signals{Reversible: true, BlastRadius: 2},
		reasonEmpty: true,
	},
	{
		name:      "case-insensitive: PAYMENT / Migrations still match",
		paths:     []string{"SRC/PAYMENT/Charge.go", "DB/Migrations/V2.SQL"},
		want:      Signals{TouchesPayment: true, TouchesMigration: true, Reversible: false, BlastRadius: 2},
		reasonHas: []string{"payment", "migration"},
	},
	{
		name:  "all four surfaces at once",
		paths: []string{"auth/login.go", "payment/charge.go", "vault/secret.go", "migrate/up.sql"},
		want: Signals{
			TouchesPayment:   true,
			TouchesAuth:      true,
			TouchesSecrets:   true,
			TouchesMigration: true,
			Reversible:       false,
			BlastRadius:      4,
		},
		reasonHas: []string{"payment", "auth", "secrets", "migration"},
	},
}

// TestFromChangedPaths_Heuristics runs the mapping table, asserting the derived
// Signals AND that ProdTraffic is NEVER set (a path cannot prove prod exposure).
func TestFromChangedPaths_Heuristics(t *testing.T) {
	for _, c := range fromPathsCases {
		t.Run(c.name, func(t *testing.T) {
			got, reasons := FromChangedPaths(c.paths)
			if got != c.want {
				t.Errorf("FromChangedPaths(%v) = %+v, want %+v", c.paths, got, c.want)
			}
			if got.ProdTraffic {
				t.Errorf("FromChangedPaths(%v) set ProdTraffic; a path must never prove prod exposure", c.paths)
			}
			joined := strings.Join(reasons, " | ")
			if c.reasonEmpty && len(reasons) != 0 {
				t.Errorf("FromChangedPaths(%v) reasons = %v, want none", c.paths, reasons)
			}
			for _, sub := range c.reasonHas {
				if !strings.Contains(joined, sub) {
					t.Errorf("FromChangedPaths(%v) reasons = %q, want substring %q", c.paths, joined, sub)
				}
			}
		})
	}
}

// The reason string must NAME the offending path so the report is auditable, and
// it must pick the FIRST matching path in input order (deterministic).
func TestFromChangedPaths_ReasonNamesFirstPath(t *testing.T) {
	_, reasons := FromChangedPaths([]string{"a/no-match.go", "b/payment.go", "c/billing.go"})
	if len(reasons) != 1 {
		t.Fatalf("reasons = %v, want exactly one (payment)", reasons)
	}
	if !strings.Contains(reasons[0], "b/payment.go") {
		t.Errorf("reason = %q, want it to name the FIRST matching path b/payment.go", reasons[0])
	}
}

// END-TO-END through the existing Classify contract: the auto-derived Signals
// must drive the SAME levels the prompt expects — a payment path classifies at
// least high; a payment + migration (irreversible) set classifies critical (the
// level that trips safety_override -> Opus). This proves the producer feeds the
// stable contract without re-mapping it.
func TestFromChangedPaths_FeedsClassify(t *testing.T) {
	payOnly, _ := FromChangedPaths([]string{"src/payment/charge.go"})
	if lvl, _ := Classify(payOnly); Rank(lvl) < Rank(High) {
		t.Errorf("payment path classifies %q, want >= high", lvl)
	}
	payMig, _ := FromChangedPaths([]string{"src/payment/charge.go", "db/migrations/001.sql"})
	if lvl, reason := Classify(payMig); lvl != Critical {
		t.Errorf("payment+migration(irreversible) classifies %q (%s), want critical", lvl, reason)
	}
	// An ordinary multi-file change stays low (auto only raises on evidence).
	plain, _ := FromChangedPaths([]string{"src/a.go", "src/b.go"})
	if lvl, _ := Classify(plain); lvl != Low && lvl != Medium {
		t.Errorf("ordinary change classifies %q, want low/medium (no sensitive surface)", lvl)
	}
}
