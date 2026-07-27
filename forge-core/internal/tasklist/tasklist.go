// Package tasklist parses and validates the planner agent's machine-readable
// TASK_LIST contract. It is deliberately independent of prompts and executors:
// callers hand it plain agent text and receive a validated dependency plan.
package tasklist

import (
	"fmt"
	"path/filepath"
	"strings"
)

const marker = "TASK_LIST:"

// Task is one atomic planner item.
type Task struct {
	ID         string
	Title      string
	Acceptance string
	Files      []string
	DependsOn  []string
	Model      string
	Roadmap    string
}

// Plan contains tasks in dependency-safe, stable topological order.
type Plan struct {
	Tasks []Task
}

// Parse finds the final TASK_LIST block, validates its schema and dependency
// graph, and returns a stable topological plan. An empty block is a valid plan.
func Parse(text string) (Plan, error) {
	lines := strings.Split(strings.ReplaceAll(text, "\r\n", "\n"), "\n")
	start := lastMarker(lines)
	if start < 0 {
		return Plan{}, fmt.Errorf("missing %s block", marker)
	}
	var tasks []Task
	for i, raw := range lines[start+1:] {
		line := strings.TrimSpace(raw)
		if line == "" {
			continue
		}
		task, err := parseLine(line)
		if err != nil {
			return Plan{}, fmt.Errorf("TASK_LIST line %d: %w", start+i+2, err)
		}
		tasks = append(tasks, task)
	}
	return validateAndOrder(tasks)
}

func lastMarker(lines []string) int {
	found := -1
	for i, line := range lines {
		if strings.TrimSpace(line) == marker {
			found = i
		}
	}
	return found
}

func parseLine(line string) (Task, error) {
	if !strings.HasPrefix(line, "- [ ] ") {
		return Task{}, fmt.Errorf("expected '- [ ] TNNN: ...', got %q", line)
	}
	parts := strings.SplitN(strings.TrimPrefix(line, "- [ ] "), " — ", 6)
	if len(parts) != 6 {
		return Task{}, fmt.Errorf("expected title plus acceptance/files/depends_on/model/roadmap fields")
	}
	id, title, ok := strings.Cut(parts[0], ": ")
	if !ok || !validID(id) || strings.TrimSpace(title) == "" {
		return Task{}, fmt.Errorf("invalid task head %q", parts[0])
	}
	acceptance, err := field(parts[1], "acceptance")
	if err != nil {
		return Task{}, err
	}
	filesRaw, err := field(parts[2], "files")
	if err != nil {
		return Task{}, err
	}
	depsRaw, err := field(parts[3], "depends_on")
	if err != nil {
		return Task{}, err
	}
	model, err := field(parts[4], "model")
	if err != nil {
		return Task{}, err
	}
	roadmap, err := field(parts[5], "roadmap")
	if err != nil {
		return Task{}, err
	}
	files, err := parseFiles(filesRaw)
	if err != nil {
		return Task{}, err
	}
	deps, err := parseDependencies(depsRaw)
	if err != nil {
		return Task{}, err
	}
	return buildTask(id, title, acceptance, roadmap, model, files, deps)
}

func buildTask(id, title, acceptance, roadmap, model string, files, deps []string) (Task, error) {
	if model != "haiku" && model != "sonnet" && model != "opus" {
		return Task{}, fmt.Errorf("model must be haiku, sonnet, or opus, got %q", model)
	}
	acceptance = strings.Trim(strings.TrimSpace(acceptance), `"`)
	if acceptance == "" || strings.TrimSpace(roadmap) == "" {
		return Task{}, fmt.Errorf("acceptance and roadmap must be non-empty")
	}
	return Task{
		ID: id, Title: strings.TrimSpace(title), Acceptance: acceptance,
		Files: files, DependsOn: deps, Model: model, Roadmap: strings.TrimSpace(roadmap),
	}, nil
}

func field(part, name string) (string, error) {
	prefix := name + ": "
	if !strings.HasPrefix(part, prefix) {
		return "", fmt.Errorf("expected %s field, got %q", name, part)
	}
	value := strings.TrimSpace(strings.TrimPrefix(part, prefix))
	if value == "" {
		return "", fmt.Errorf("%s must be non-empty", name)
	}
	return value, nil
}

func validID(id string) bool {
	if len(id) != 4 || id[0] != 'T' {
		return false
	}
	for _, r := range id[1:] {
		if r < '0' || r > '9' {
			return false
		}
	}
	return true
}

func parseFiles(raw string) ([]string, error) {
	var out []string
	for _, item := range strings.Split(raw, ",") {
		name := strings.TrimSpace(item)
		clean := filepath.Clean(name)
		if name == "" || filepath.IsAbs(name) || clean == ".." ||
			strings.HasPrefix(clean, ".."+string(filepath.Separator)) {
			return nil, fmt.Errorf("files contains unsafe path %q", name)
		}
		out = append(out, filepath.ToSlash(clean))
	}
	return out, nil
}

func parseDependencies(raw string) ([]string, error) {
	if raw == "none" {
		return nil, nil
	}
	var out []string
	seen := map[string]bool{}
	for _, item := range strings.Split(raw, ",") {
		id := strings.TrimSpace(item)
		if !validID(id) {
			return nil, fmt.Errorf("invalid depends_on task id %q", id)
		}
		if !seen[id] {
			seen[id] = true
			out = append(out, id)
		}
	}
	return out, nil
}

func validateAndOrder(tasks []Task) (Plan, error) {
	byID := make(map[string]Task, len(tasks))
	for _, task := range tasks {
		if _, exists := byID[task.ID]; exists {
			return Plan{}, fmt.Errorf("duplicate task id %s", task.ID)
		}
		byID[task.ID] = task
	}
	indegree := make(map[string]int, len(tasks))
	next := make(map[string][]string, len(tasks))
	for _, task := range tasks {
		for _, dep := range task.DependsOn {
			if dep == task.ID {
				return Plan{}, fmt.Errorf("task %s depends on itself", task.ID)
			}
			if _, exists := byID[dep]; !exists {
				return Plan{}, fmt.Errorf("task %s depends on unknown task %s", task.ID, dep)
			}
			indegree[task.ID]++
			next[dep] = append(next[dep], task.ID)
		}
	}
	ordered := make([]Task, 0, len(tasks))
	done := map[string]bool{}
	for len(ordered) < len(tasks) {
		picked := ""
		for _, task := range tasks {
			if !done[task.ID] && indegree[task.ID] == 0 {
				picked = task.ID
				break
			}
		}
		if picked == "" {
			return Plan{}, fmt.Errorf("dependency cycle detected")
		}
		done[picked] = true
		ordered = append(ordered, byID[picked])
		for _, id := range next[picked] {
			indegree[id]--
		}
	}
	return Plan{Tasks: ordered}, nil
}

// Render returns a normalized TASK_LIST block in dependency-safe order.
func Render(plan Plan) string {
	var b strings.Builder
	b.WriteString(marker)
	for _, task := range plan.Tasks {
		deps := "none"
		if len(task.DependsOn) > 0 {
			deps = strings.Join(task.DependsOn, ", ")
		}
		fmt.Fprintf(&b,
			"\n- [ ] %s: %s — acceptance: %q — files: %s — depends_on: %s — model: %s — roadmap: %s",
			task.ID, task.Title, task.Acceptance, strings.Join(task.Files, ", "),
			deps, task.Model, task.Roadmap)
	}
	return b.String()
}
