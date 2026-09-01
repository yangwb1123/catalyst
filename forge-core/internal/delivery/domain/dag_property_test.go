package domain

import (
	"errors"
	"math/rand"
	"reflect"
	"testing"
)

func TestTopologicalOrderIsDeterministicAcrossRandomPermutations(t *testing.T) {
	for seed := int64(1); seed <= 128; seed++ {
		random := rand.New(rand.NewSource(seed))
		items := randomizedDAG(random, 2+random.Intn(23))
		before := cloneWorkItems(items)
		want, err := TopologicalOrder(items)
		if err != nil {
			t.Fatalf("seed %d baseline: %v", seed, err)
		}
		assertTopologicalOrder(t, seed, items, want)
		for permutation := 0; permutation < 8; permutation++ {
			candidate := cloneWorkItems(items)
			random.Shuffle(len(candidate), func(i, j int) { candidate[i], candidate[j] = candidate[j], candidate[i] })
			for index := range candidate {
				random.Shuffle(len(candidate[index].Dependencies), func(i, j int) {
					candidate[index].Dependencies[i], candidate[index].Dependencies[j] =
						candidate[index].Dependencies[j], candidate[index].Dependencies[i]
				})
			}
			got, err := TopologicalOrder(candidate)
			if err != nil || !reflect.DeepEqual(got, want) {
				t.Fatalf("seed %d permutation %d: order=%v want=%v err=%v", seed, permutation, got, want, err)
			}
		}
		if !reflect.DeepEqual(items, before) {
			t.Fatalf("seed %d: TopologicalOrder mutated input", seed)
		}
	}
}

func TestTopologicalOrderRejectsInjectedRandomCycles(t *testing.T) {
	for seed := int64(1); seed <= 64; seed++ {
		random := rand.New(rand.NewSource(seed * 7919))
		items := randomizedDAG(random, 3+random.Intn(20))
		child := 1 + random.Intn(len(items)-1)
		parentID := items[child].Dependencies[0]
		parent := workItemIndex(items, parentID)
		items[parent].Dependencies = append(items[parent].Dependencies, items[child].WorkItemID)
		if _, err := TopologicalOrder(items); !errors.Is(err, ErrInvalidDomain) {
			t.Fatalf("seed %d injected cycle error = %v", seed, err)
		}
	}
}

func TestTopologicalOrderEnforcesItemAndEdgeBounds(t *testing.T) {
	items := bareWorkItems(maxWorkItems)
	if order, err := TopologicalOrder(items); err != nil || len(order) != maxWorkItems {
		t.Fatalf("maximum item DAG = %d, %v", len(order), err)
	}
	if _, err := TopologicalOrder(bareWorkItems(maxWorkItems + 1)); !errors.Is(err, ErrInvalidDomain) {
		t.Fatalf("over-bound item error = %v", err)
	}
	if _, err := TopologicalOrder(denseDAG(maxDependencyEdges)); err != nil {
		t.Fatalf("maximum edge DAG: %v", err)
	}
	if _, err := TopologicalOrder(denseDAG(maxDependencyEdges + 1)); !errors.Is(err, ErrInvalidDomain) {
		t.Fatalf("over-bound edge error = %v", err)
	}
}

func TestTopologicalOrderRejectsEveryMalformedDependencyRelation(t *testing.T) {
	valid := bareWorkItems(34)
	tests := []struct {
		name   string
		mutate func([]WorkItem)
	}{
		{"duplicate item", func(v []WorkItem) { v[1].WorkItemID = v[0].WorkItemID }},
		{"invalid item", func(v []WorkItem) { v[0].WorkItemID = "bad" }},
		{"missing", func(v []WorkItem) { v[1].Dependencies = []string{testID("wki", 99)} }},
		{"self", func(v []WorkItem) { v[1].Dependencies = []string{v[1].WorkItemID} }},
		{"duplicate edge", func(v []WorkItem) { v[1].Dependencies = []string{v[0].WorkItemID, v[0].WorkItemID} }},
		{"item edge bound", func(v []WorkItem) { v[33].Dependencies = numberedIDs("wki", maxItemListEntries+1) }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			items := cloneWorkItems(valid)
			test.mutate(items)
			if _, err := TopologicalOrder(items); !errors.Is(err, ErrInvalidDomain) {
				t.Fatalf("malformed topology error = %v", err)
			}
		})
	}
}

func TestTopologicalOrderUsesLexicalPriorityForEveryReadySet(t *testing.T) {
	first := testID("wki", 1)
	items := []WorkItem{
		{WorkItemID: testID("wki", 3)},
		{WorkItemID: testID("wki", 2), Dependencies: []string{first}},
		{WorkItemID: first},
		{WorkItemID: testID("wki", 4)},
	}
	want := []string{first, testID("wki", 2), testID("wki", 3), testID("wki", 4)}
	order, err := TopologicalOrder(items)
	if err != nil || !reflect.DeepEqual(order, want) {
		t.Fatalf("lexical ready order = %v, want %v, error %v", order, want, err)
	}
}

func TestTopologicalOrderRejectsOversizedDependenciesBeforeLargeAllocation(t *testing.T) {
	items := bareWorkItems(2)
	items[1].Dependencies = make([]string, 4096)
	result := testing.Benchmark(func(benchmark *testing.B) {
		for index := 0; index < benchmark.N; index++ {
			_, _ = TopologicalOrder(items)
		}
	})
	if bytes := result.AllocedBytesPerOp(); bytes > 16*1024 {
		t.Fatalf("oversized dependency rejection allocated %d bytes/op", bytes)
	}
}

func randomizedDAG(random *rand.Rand, count int) []WorkItem {
	serials := random.Perm(count)
	items := make([]WorkItem, count)
	for index := range items {
		items[index].WorkItemID = testID("wki", serials[index]+1)
		if index == 0 {
			continue
		}
		candidates := random.Perm(index)
		dependencyCount := 1 + random.Intn(minimum(index, maxItemListEntries))
		for _, candidate := range candidates[:dependencyCount] {
			items[index].Dependencies = append(items[index].Dependencies, items[candidate].WorkItemID)
		}
	}
	return items
}

func cloneWorkItems(items []WorkItem) []WorkItem {
	result := append([]WorkItem(nil), items...)
	for index := range result {
		result[index].Dependencies = append([]string(nil), items[index].Dependencies...)
	}
	return result
}

func assertTopologicalOrder(t *testing.T, seed int64, items []WorkItem, order []string) {
	t.Helper()
	positions := make(map[string]int, len(order))
	for index, itemID := range order {
		positions[itemID] = index
	}
	for _, item := range items {
		for _, dependency := range item.Dependencies {
			if positions[dependency] >= positions[item.WorkItemID] {
				t.Fatalf("seed %d: dependency %s appears after %s", seed, dependency, item.WorkItemID)
			}
		}
	}
}

func bareWorkItems(count int) []WorkItem {
	items := make([]WorkItem, count)
	for index := range items {
		items[index].WorkItemID = testID("wki", index+1)
	}
	return items
}

func denseDAG(edgeCount int) []WorkItem {
	items := bareWorkItems(maxWorkItems)
	remaining := edgeCount
	for item := 1; item < len(items) && remaining > 0; item++ {
		count := minimum(item, maxItemListEntries)
		count = minimum(count, remaining)
		for dependency := 0; dependency < count; dependency++ {
			items[item].Dependencies = append(items[item].Dependencies, items[dependency].WorkItemID)
		}
		remaining -= count
	}
	if remaining != 0 {
		panic("dense DAG helper cannot represent requested edge count")
	}
	return items
}

func workItemIndex(items []WorkItem, itemID string) int {
	for index := range items {
		if items[index].WorkItemID == itemID {
			return index
		}
	}
	panic("randomized DAG contains an unknown item")
}

func minimum(left, right int) int {
	if left < right {
		return left
	}
	return right
}
