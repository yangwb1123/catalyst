package projectsnapshot

import "testing"

func TestFixedPathPolicyExactRulesAndPrecedence(t *testing.T) {
	tests := map[string]string{
		".git/config": "control_path", ".FORGE/state": "control_path",
		".git/secrets/token": "control_path", ".SSH/config": "sensitive_path",
		"src/.aws/config": "sensitive_path", "secrets": "sensitive_path",
		"src/SECRETS/token": "sensitive_path", ".env": "sensitive_path",
		"config/.ENV.local": "sensitive_path", ".netrc": "sensitive_path",
		".npmrc": "sensitive_path", ".pypirc": "sensitive_path",
		".dockercfg": "sensitive_path", "kubeconfig": "sensitive_path",
		"credentials": "sensitive_path", "credentials.json": "sensitive_path",
		"service-account.json": "sensitive_path", "id_ed25519": "sensitive_path",
		"cert.pem": "sensitive_path", "store.keystore": "sensitive_path",
		"secrets.json": "", "data.tfstate": "", "archive.age": "",
		"secret/value": "", "private/value": "", "vault/value": "",
		"Ｓecrets/value": "", "source/main.go": "",
	}
	for path, want := range tests {
		if got := protectedPathReason(path); got != want {
			t.Errorf("protectedPathReason(%q) = %q, want %q", path, got, want)
		}
	}
}
