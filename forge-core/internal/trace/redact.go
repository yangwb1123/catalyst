package trace

import "regexp"

var traceRedactors = []struct {
	pattern *regexp.Regexp
	replace string
}{
	{
		// Match both bare credential keys (API_KEY) and vendor/application
		// qualified keys (ANTHROPIC_API_KEY, GITHUB_TOKEN,
		// AWS_SECRET_ACCESS_KEY). Requiring underscore/hyphen-separated key
		// components avoids treating an unrelated word containing "secret" as a
		// credential name, while allowing suffixes such as _ACCESS_KEY.
		pattern: regexp.MustCompile(`(?i)\b((?:[a-z0-9]+[_-])*(?:api[_-]?key|access[_-]?token|auth[_-]?token|token|secret|password|authorization)(?:[_-][a-z0-9]+)*)(\s*[:=]\s*)(?:"[^"]*"|'[^']*'|[^\s,;]+)`),
		replace: `${1}${2}[REDACTED]`,
	},
	{
		pattern: regexp.MustCompile(`(?i)\b(bearer)\s+[A-Za-z0-9._~+/=-]+`),
		replace: `${1} [REDACTED]`,
	},
	{
		pattern: regexp.MustCompile(`\bsk-[A-Za-z0-9_-]{12,}\b`),
		replace: `[REDACTED]`,
	},
	{
		pattern: regexp.MustCompile(`\b(?:gh[pousr]_|github_pat_)[A-Za-z0-9_]{12,}\b`),
		replace: `[REDACTED]`,
	},
	{
		pattern: regexp.MustCompile(`\bAKIA[0-9A-Z]{16}\b`),
		replace: `[REDACTED]`,
	},
}

// redactSensitive keeps common credentials out of the durable trace detail.
// It is deliberately deterministic and narrow: trace structure remains fully
// queryable, while values matching well-known credential shapes are removed.
func redactSensitive(detail string) string {
	for _, redactor := range traceRedactors {
		detail = redactor.pattern.ReplaceAllString(detail, redactor.replace)
	}
	return detail
}
