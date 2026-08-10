package localcommandobservationproducer

import (
	"fmt"
	"path/filepath"
	"sort"
	"strings"
)

const maxEnvironmentVariables = 256

var secretEnvironmentFragments = []string{
	"API_KEY", "AUTH", "BEARER", "COOKIE", "CREDENTIAL", "CREDENTIALS",
	"OAUTH", "PASSWD", "PASSWORD", "PRIVATE_KEY", "SECRET", "SESSION", "TOKEN",
}

var secretEnvironmentPrefixes = []string{
	"ANTHROPIC_", "AWS_", "AZURE_", "CLOUDFLARE_", "DIGITALOCEAN_", "DOCKER_AUTH",
	"GCP_", "GCLOUD_", "GITHUB_", "GITLAB_", "GOOGLE_", "KUBE", "OCI_", "OPENAI_",
	"SSH_", "GPG_", "VAULT_",
}

// environmentSnapshot constructs the exact child environment: every retained
// non-secret parent variable, sorted by name with duplicates rejected.
func environmentSnapshot(parent []string) (EnvironmentManifest, string, []string, error) {
	variables, err := scrubEnvironment(parent)
	if err != nil {
		return EnvironmentManifest{}, "", nil, err
	}
	manifest := EnvironmentManifest{
		APIVersion: EnvironmentAPIVersion, Canonicalization: Canonicalization,
		ProfileID: environmentProfileID, Variables: variables,
	}
	_, digest, err := digestManifest(environmentDigestDomain, manifest)
	if err != nil {
		return EnvironmentManifest{}, "", nil, err
	}
	return manifest, digest, environmentStrings(variables), nil
}

func scrubEnvironment(parent []string) ([]EnvironmentVariable, error) {
	if len(parent) > maxEnvironmentVariables {
		return nil, fmt.Errorf("parent environment exceeds %d variables", maxEnvironmentVariables)
	}
	seen := make(map[string]struct{}, len(parent))
	retained := make([]EnvironmentVariable, 0, len(parent))
	for _, entry := range parent {
		name, value, ok := strings.Cut(entry, "=")
		if !ok || !validEnvironmentName(name) {
			return nil, fmt.Errorf("invalid environment entry name %q", name)
		}
		if _, exists := seen[name]; exists {
			return nil, fmt.Errorf("duplicate environment variable %q", name)
		}
		seen[name] = struct{}{}
		if secretEnvironmentName(name) {
			continue
		}
		if err := validateText("environment "+name, value, true); err != nil {
			return nil, err
		}
		if name == "PATH" {
			if _, err := scrubbedPathDirectories(value); err != nil {
				return nil, err
			}
		}
		retained = append(retained, EnvironmentVariable{Name: name, Value: value})
	}
	sort.Slice(retained, func(i, j int) bool { return retained[i].Name < retained[j].Name })
	if !hasEnvironmentVariable(retained, "PATH") {
		return nil, fmt.Errorf("scrubbed environment must contain PATH")
	}
	return retained, nil
}

// scrubbedPathDirectories validates the complete Unix-family PATH before any
// caller searches it. In particular, a usable executable in an early entry
// must not mask a malformed later entry.
func scrubbedPathDirectories(pathValue string) ([]string, error) {
	directories := filepath.SplitList(pathValue)
	if len(directories) == 0 {
		return nil, fmt.Errorf("PATH entry %q must be a normalized absolute path", "")
	}
	for _, directory := range directories {
		if directory == "" || !filepath.IsAbs(directory) || filepath.Clean(directory) != directory {
			return nil, fmt.Errorf("PATH entry %q must be a normalized absolute path", directory)
		}
	}
	return directories, nil
}

func validEnvironmentName(name string) bool {
	if name == "" || name[0] != '_' && !asciiLetter(name[0]) {
		return false
	}
	for index := 1; index < len(name); index++ {
		character := name[index]
		if character != '_' && !asciiLetter(character) && (character < '0' || character > '9') {
			return false
		}
	}
	return true
}

func asciiLetter(value byte) bool {
	return value >= 'A' && value <= 'Z' || value >= 'a' && value <= 'z'
}

func secretEnvironmentName(name string) bool {
	upper := strings.ToUpper(name)
	if upper == "HTTP_PROXY" || upper == "HTTPS_PROXY" || upper == "ALL_PROXY" || upper == "NO_PROXY" {
		return true
	}
	for _, prefix := range secretEnvironmentPrefixes {
		if strings.HasPrefix(upper, prefix) {
			return true
		}
	}
	for _, fragment := range secretEnvironmentFragments {
		if strings.Contains(upper, fragment) {
			return true
		}
	}
	return false
}

func environmentStrings(variables []EnvironmentVariable) []string {
	result := make([]string, len(variables))
	for index, variable := range variables {
		result[index] = variable.Name + "=" + variable.Value
	}
	return result
}

func hasEnvironmentVariable(variables []EnvironmentVariable, name string) bool {
	for _, variable := range variables {
		if variable.Name == name {
			return true
		}
	}
	return false
}

func environmentValue(manifest EnvironmentManifest, name string) (string, bool) {
	for _, variable := range manifest.Variables {
		if variable.Name == name {
			return variable.Value, true
		}
	}
	return "", false
}
