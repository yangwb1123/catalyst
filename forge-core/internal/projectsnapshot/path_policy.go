package projectsnapshot

import "strings"

var sensitiveBasenames = map[string]struct{}{
	".dockercfg": {}, ".env": {}, ".netrc": {}, ".npmrc": {}, ".pypirc": {},
	"credentials": {}, "credentials.json": {}, "id_dsa": {}, "id_ecdsa": {},
	"id_ed25519": {}, "id_rsa": {}, "kubeconfig": {}, "service-account.json": {},
}

var sensitiveSuffixes = []string{
	".jks", ".key", ".keystore", ".p12", ".pfx", ".pem",
}

var sensitiveSegments = map[string]struct{}{
	".aws": {}, ".azure": {}, ".gnupg": {}, ".ssh": {}, "secrets": {},
}

func protectedPathReason(path string) string {
	components := strings.Split(foldASCII(path), "/")
	for _, component := range components {
		if component == ".git" || component == ".forge" {
			return "control_path"
		}
	}
	for _, component := range components {
		if _, sensitive := sensitiveSegments[component]; sensitive {
			return "sensitive_path"
		}
	}
	base := components[len(components)-1]
	if _, sensitive := sensitiveBasenames[base]; sensitive || strings.HasPrefix(base, ".env.") {
		return "sensitive_path"
	}
	for _, suffix := range sensitiveSuffixes {
		if strings.HasSuffix(base, suffix) {
			return "sensitive_path"
		}
	}
	return ""
}
