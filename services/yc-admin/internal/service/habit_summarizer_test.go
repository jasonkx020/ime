package service

import (
	"testing"

	"yc-admin/internal/store"
)

func TestHabitSummarizerEnPairs(t *testing.T) {
	sum := HabitSummarizer{}
	out := sum.Summarize(summarizerInput{
		Lang: "en",
		TopWords: []store.AggregateRow{
			{Key: "th", Word: "thanks", Lang: "en", Count: 10, AvgPos: 2},
			{Key: "th", Word: "the", Lang: "en", Count: 2, AvgPos: 1},
		},
		Selects: 100,
		AvgPos:  2.5,
	})
	if len(out.PreferPairs) == 0 {
		t.Fatal("expected prefer_pairs for en")
	}
	foundDemote := false
	for _, d := range out.Demote {
		if d.Word == "the" && d.QueryKey == "th" {
			foundDemote = true
		}
	}
	if !foundDemote {
		t.Fatalf("expected demote for weaker rival 'the', got %#v", out.Demote)
	}
	hasLang := false
	for _, tag := range out.Tags {
		if tag == "lang_en" {
			hasLang = true
		}
	}
	if !hasLang {
		t.Fatalf("expected lang_en tag, got %v", out.Tags)
	}
}
