package service

import (
	"encoding/json"
	"strings"

	"yc-admin/internal/model"
	"yc-admin/internal/store"
)

// HabitSummarizer turns aggregated habits into prefer_pairs / demote / tags.
// Production can swap the rule body for an LLM call; schema stays identical per lang.
type HabitSummarizer struct{}

type summarizerInput struct {
	Lang     string
	TopWords []store.AggregateRow
	TopKeys  []store.AggregateRow
	AvgPos   float64
	Selects  int64
}

type summarizerOutput struct {
	PreferPairs []model.PreferPair
	Demote      []model.WordBoost
	Tags        []string
}

func (h HabitSummarizer) Summarize(in summarizerInput) summarizerOutput {
	out := summarizerOutput{Tags: []string{}}
	if in.Lang != "" {
		out.Tags = append(out.Tags, "lang_"+in.Lang)
	}
	if in.Selects < 20 {
		out.Tags = append(out.Tags, "new_user")
	} else if in.Selects > 500 {
		out.Tags = append(out.Tags, "power_user")
	}
	if in.AvgPos >= 3 {
		out.Tags = append(out.Tags, "needs_rerank")
	}

	// Same query_key competing words → demote lower-count rivals lightly.
	byKey := map[string][]store.AggregateRow{}
	for _, w := range in.TopWords {
		if w.Lang != "" && in.Lang != "" && w.Lang != in.Lang {
			continue
		}
		byKey[w.Key] = append(byKey[w.Key], w)
	}
	for key, rows := range byKey {
		if len(rows) < 2 || key == "" {
			continue
		}
		best := rows[0]
		for _, r := range rows[1:] {
			if r.Count > best.Count {
				best = r
			}
		}
		for _, r := range rows {
			if r.Word == best.Word {
				continue
			}
			if r.Count*2 >= best.Count {
				continue // close race — don't demote
			}
			out.Demote = append(out.Demote, model.WordBoost{
				QueryKey: key,
				Pinyin:   key,
				Word:     r.Word,
				Boost:    -1.0,
				Freq:     r.Count,
				Lang:     in.Lang,
			})
		}
	}

	// Language-specific few-shot style pairs from frequent words (rule stand-in for LLM).
	out.PreferPairs = append(out.PreferPairs, heuristicPairs(in.Lang, in.TopWords)...)

	if len(out.PreferPairs) > 40 {
		out.PreferPairs = out.PreferPairs[:40]
	}
	if len(out.Demote) > 40 {
		out.Demote = out.Demote[:40]
	}
	return out
}

func heuristicPairs(lang string, words []store.AggregateRow) []model.PreferPair {
	seen := map[string]bool{}
	var pairs []model.PreferPair
	add := func(prev, next string, delta float64) {
		k := prev + "\t" + next
		if prev == "" || next == "" || seen[k] {
			return
		}
		seen[k] = true
		pairs = append(pairs, model.PreferPair{Prev: prev, Next: next, Delta: delta})
	}

	switch lang {
	case "en":
		add("thank", "you", 1.5)
		add("good", "morning", 1.2)
		add("how", "are", 1.0)
	case "zh":
		add("你", "好", 1.5)
		add("我", "们", 1.2)
	case "vi":
		add("xin", "chào", 1.5)
		add("cảm", "ơn", 1.2)
	case "th":
		add("สวัสดี", "ครับ", 1.2)
		add("ขอบคุณ", "ครับ", 1.2)
	}

	// From top words: if word has a space, split into prefer pair.
	for _, w := range words {
		parts := strings.Fields(w.Word)
		if len(parts) == 2 {
			add(parts[0], parts[1], 1.0)
		}
	}
	return pairs
}

func encodePairs(pairs []model.PreferPair) string {
	b, _ := json.Marshal(pairs)
	if len(b) == 0 {
		return "[]"
	}
	return string(b)
}

func encodeBoosts(boosts []model.WordBoost) string {
	b, _ := json.Marshal(boosts)
	if len(b) == 0 {
		return "[]"
	}
	return string(b)
}
