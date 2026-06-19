package risk

// risk_diff.go — AUTOMATIC feature extraction for the risk classifier. risk.go's
// header is explicit that v1 takes Signals as EXPLICIT input and that deriving
// them from a diff is "downstream wiring / v3". This file is the first, honest
// step toward that producer: it derives Signals from the SET OF CHANGED FILE
// PATHS using path-substring heuristics, so a caller with a git diff gets a
// better-than-hand-filled signal automatically. The output still flows into the
// SAME Classify contract — this only fills the inputs, it does not re-map levels.
//
// HONESTY — what this is and is NOT (do not oversell it):
//   - Path-substring matching is a COARSE heuristic. It WILL miss (a payment code
//     path named "ledger.go" with no "payment" in it) and it WILL over-match (a
//     test fixture or comment that merely contains "auth"). It reads only the
//     PATH, never the file content or the call graph.
//   - Precise extraction needs real signal: AST / call-graph reachability to know
//     a payment code path was truly touched, CODEOWNERS to attribute a surface,
//     and the actual deployment topology to know prod_traffic. That is v3. v1
//     deliberately ships the cheap proxy because it still beats a human guessing.
//   - BY DESIGN THIS ONLY RAISES RISK, NEVER LOWERS IT. It sets sensitive-surface
//     and migration flags when matched; it leaves ProdTraffic FALSE always
//     (a path cannot honestly prove production exposure) and keeps Reversible
//     true except for migrations. A MISS therefore under-states risk, which the
//     human corrects with an explicit --touches-* override (route takes the
//     HIGHER of auto and manual — auto can never silently lower a declared risk).
//   - ProdTraffic is intentionally left to explicit declaration: inferring it
//     from a path would be a guess that could ESCALATE (irreversible + prod ->
//     critical), and the rule above is "only raise on EVIDENCE", not on a guess.

import "strings"

// surface pairs a set of path-substring needles with the human-readable reason
// emitted when any of them matches. Needles are matched case-insensitively
// against the lower-cased path. Ordered so reasons come out deterministically.
type surface struct {
	needles []string
	reason  string
}

// paymentNeedles etc. are the substring heuristics per sensitive surface. They
// mirror the four Signals booleans risk.Classify already understands. Kept as
// named package vars (not inline) so the mapping is auditable in one place and
// the matcher loop stays tiny.
var (
	paymentNeedles   = []string{"payment", "billing", "charge", "invoice"}
	authNeedles      = []string{"auth", "authz", "authn", "login", "session", "permission", "rbac"}
	secretNeedles    = []string{"secret", "credential", "vault", ".key", ".pem"}
	migrationNeedles = []string{"migration", "migrate", "schema", ".sql"}
)

// FromChangedPaths derives risk Signals from the set of changed file paths using
// path-substring heuristics, returning the Signals plus a list of human-readable
// hit reasons (one per surface that matched, naming the surface and the first
// path that tripped it) for observability. It is PURE: no I/O, deterministic,
// order-stable.
//
// Semantics (see the honesty note above): matching a sensitive needle RAISES the
// corresponding flag; migration ALSO forces Reversible=false (a schema/data
// migration rarely rolls back cleanly). BlastRadius is the count of changed
// paths. Reversible defaults true (the safe, risk-LOWERING assumption, only
// overridden upward by migration); ProdTraffic is left FALSE — a path cannot
// honestly establish production exposure. The empty input yields the zero
// Signals value (Reversible=false there too, which Classify reads as low).
func FromChangedPaths(paths []string) (Signals, []string) {
	if len(paths) == 0 {
		return Signals{}, nil // no changed paths => zero-value Signals (Classify -> low)
	}
	s := Signals{Reversible: true, BlastRadius: len(paths)}
	var reasons []string
	if p, ok := firstMatch(paths, paymentNeedles); ok {
		s.TouchesPayment = true
		reasons = append(reasons, "payment surface ("+p+")")
	}
	if p, ok := firstMatch(paths, authNeedles); ok {
		s.TouchesAuth = true
		reasons = append(reasons, "auth surface ("+p+")")
	}
	if p, ok := firstMatch(paths, secretNeedles); ok {
		s.TouchesSecrets = true
		reasons = append(reasons, "secrets surface ("+p+")")
	}
	if p, ok := firstMatch(paths, migrationNeedles); ok {
		s.TouchesMigration = true
		s.Reversible = false // migrations rarely roll back cleanly -> raise risk
		reasons = append(reasons, "migration ("+p+", irreversible)")
	}
	return s, reasons
}

// firstMatch reports the first path (in input order) that contains any needle,
// matched case-insensitively, so the reason string can name the offending path
// deterministically. ok is false when no path matches.
func firstMatch(paths, needles []string) (string, bool) {
	for _, p := range paths {
		lp := strings.ToLower(p)
		for _, n := range needles {
			if strings.Contains(lp, n) {
				return p, true
			}
		}
	}
	return "", false
}
