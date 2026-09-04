package service

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"yc-admin/internal/model"
	"yc-admin/internal/store"
)

type Services struct {
	Store      *store.Store
	PublicBase string
}

func (s *Services) CreateDraft(ctx context.Context, p model.LangPack) (*model.LangPack, error) {
	p.PackID = strings.TrimSpace(p.PackID)
	p.Lang = strings.TrimSpace(p.Lang)
	if p.PackID == "" || p.Lang == "" {
		return nil, fmt.Errorf("pack_id and lang required")
	}
	if p.DisplayName == "" {
		p.DisplayName = p.PackID
	}
	if p.Version == 0 {
		p.Version = 1
	}
	if p.MinHostVersion == "" {
		p.MinHostVersion = "0.1.0"
	}
	p.Status = "draft"
	if err := s.Store.CreateLangPack(ctx, &p); err != nil {
		return nil, err
	}
	return &p, nil
}

func (s *Services) UpdateMeta(ctx context.Context, id int64, displayName, minHost, notes, status string) (*model.LangPack, error) {
	p, err := s.Store.GetLangPack(ctx, id)
	if err != nil {
		return nil, err
	}
	if displayName != "" {
		p.DisplayName = displayName
	}
	if minHost != "" {
		p.MinHostVersion = minHost
	}
	if notes != "" {
		p.Notes = notes
	}
	if status != "" {
		switch status {
		case "draft", "review", "published", "archived":
			p.Status = status
		default:
			return nil, fmt.Errorf("invalid status")
		}
	}
	if err := s.Store.UpdateLangPack(ctx, p); err != nil {
		return nil, err
	}
	return p, nil
}

func (s *Services) UploadArtifact(ctx context.Context, id int64, r io.Reader, filename string) (*model.LangPack, error) {
	p, err := s.Store.GetLangPack(ctx, id)
	if err != nil {
		return nil, err
	}
	if p.Status == "published" {
		return nil, fmt.Errorf("cannot replace published artifact; create a new version")
	}
	safeName := fmt.Sprintf("%s-v%d.imepack", p.PackID, p.Version)
	dest := filepath.Join(s.Store.PacksDir(), safeName)
	tmp := dest + ".tmp"
	f, err := os.Create(tmp)
	if err != nil {
		return nil, err
	}
	h := sha256.New()
	n, err := io.Copy(io.MultiWriter(f, h), r)
	cerr := f.Close()
	if err != nil {
		_ = os.Remove(tmp)
		return nil, err
	}
	if cerr != nil {
		_ = os.Remove(tmp)
		return nil, cerr
	}
	if err := os.Rename(tmp, dest); err != nil {
		_ = os.Remove(tmp)
		return nil, err
	}
	sum := hex.EncodeToString(h.Sum(nil))
	p.SHA256 = sum
	p.SizeBytes = n
	p.FileName = safeName
	if filename != "" && !strings.HasSuffix(strings.ToLower(filename), ".imepack") {
		// keep computed name; ignore odd client filenames
	}
	if p.Status == "draft" {
		p.Status = "review"
	}
	if err := s.Store.UpdateLangPack(ctx, p); err != nil {
		return nil, err
	}
	return p, nil
}

func (s *Services) Publish(ctx context.Context, id int64) (*model.LangPack, error) {
	p, err := s.Store.GetLangPack(ctx, id)
	if err != nil {
		return nil, err
	}
	if p.FileName == "" || p.SHA256 == "" {
		return nil, fmt.Errorf("upload .imepack before publish")
	}
	path := filepath.Join(s.Store.PacksDir(), p.FileName)
	if _, err := os.Stat(path); err != nil {
		return nil, fmt.Errorf("artifact missing on disk")
	}
	now := time.Now().UTC()
	p.Status = "published"
	p.PublishedAt = &now
	if err := s.Store.UpdateLangPack(ctx, p); err != nil {
		return nil, err
	}
	if _, err := s.Store.BumpCatalogVersion(ctx); err != nil {
		return nil, err
	}
	return p, nil
}

func (s *Services) Archive(ctx context.Context, id int64) (*model.LangPack, error) {
	p, err := s.Store.GetLangPack(ctx, id)
	if err != nil {
		return nil, err
	}
	p.Status = "archived"
	if err := s.Store.UpdateLangPack(ctx, p); err != nil {
		return nil, err
	}
	if _, err := s.Store.BumpCatalogVersion(ctx); err != nil {
		return nil, err
	}
	return p, nil
}

func (s *Services) BuildCatalog(ctx context.Context) (*model.Catalog, error) {
	packs, err := s.Store.ListPublishedLatest(ctx)
	if err != nil {
		return nil, err
	}
	ver, err := s.Store.CatalogVersion(ctx)
	if err != nil {
		return nil, err
	}
	cat := &model.Catalog{
		CatalogVersion: ver,
		FetchedAt:      time.Now().UTC().Unix(),
		Entries:        make([]model.CatalogEntry, 0, len(packs)),
	}
	base := strings.TrimRight(s.PublicBase, "/")
	for _, p := range packs {
		cat.Entries = append(cat.Entries, model.CatalogEntry{
			PackID:         p.PackID,
			Lang:           p.Lang,
			Version:        p.Version,
			URL:            fmt.Sprintf("%s/cdn/langpacks/%s", base, p.FileName),
			SHA256:         p.SHA256,
			SizeBytes:      uint64(p.SizeBytes),
			MinHostVersion: p.MinHostVersion,
			DisplayName:    p.DisplayName,
		})
	}
	return cat, nil
}

func (s *Services) ArtifactPath(fileName string) (string, error) {
	base := filepath.Base(fileName)
	if base != fileName || strings.Contains(base, "..") {
		return "", fmt.Errorf("invalid file name")
	}
	path := filepath.Join(s.Store.PacksDir(), base)
	if _, err := os.Stat(path); err != nil {
		return "", err
	}
	return path, nil
}

// IngestHabits stores privacy-ok events and refreshes profile + boosts for affected devices.
func (s *Services) IngestHabits(ctx context.Context, events []model.HabitEvent) (accepted int, err error) {
	filtered := make([]model.HabitEvent, 0, len(events))
	devices := map[string]struct{}{}
	for _, e := range events {
		if !e.PrivacyOK || e.DeviceID == "" || e.EventType == "" {
			continue
		}
		filtered = append(filtered, e)
		devices[e.DeviceID] = struct{}{}
	}
	if err := s.Store.InsertHabitEvents(ctx, filtered); err != nil {
		return 0, err
	}
	for id := range devices {
		if _, err := s.RebuildProfileAndBoosts(ctx, id); err != nil {
			return len(filtered), err
		}
	}
	return len(filtered), nil
}

func (s *Services) RebuildProfileAndBoosts(ctx context.Context, deviceID string) (*model.PersonalizationPack, error) {
	since := time.Now().UTC().Add(-30 * 24 * time.Hour)
	selects, backspaces, langs, packs, topKeys, topWords, avgPos, err := s.Store.AggregateDeviceHabits(ctx, deviceID, since)
	if err != nil {
		return nil, err
	}
	totalNav := selects + backspaces
	backRate := 0.0
	if totalNav > 0 {
		backRate = float64(backspaces) / float64(totalNav)
	}

	profile := &model.UserProfile{
		DeviceID:         deviceID,
		LangPrefs:        langs,
		AvgSelectPos:     avgPos,
		SelectCount:      selects,
		BackspaceRate:    backRate,
		PreferredPackIDs: topNKeys(packs, 5),
		PersonaTags:      derivePersonaTags(avgPos, backRate, selects, langs),
	}
	for _, k := range topKeys {
		profile.TopKeys = append(profile.TopKeys, model.WordStat{Key: k.Key, Word: k.Key, Count: k.Count})
	}
	for _, w := range topWords {
		profile.TopWords = append(profile.TopWords, model.WordStat{Key: w.Key, Word: w.Word, Count: w.Count, Score: scoreBoost(w.Count, w.AvgPos)})
	}

	langJSON, _ := json.Marshal(profile.LangPrefs)
	keysJSON, _ := json.Marshal(profile.TopKeys)
	wordsJSON, _ := json.Marshal(profile.TopWords)
	packsJSON, _ := json.Marshal(profile.PreferredPackIDs)
	tagsJSON, _ := json.Marshal(profile.PersonaTags)
	if err := s.Store.UpsertProfile(ctx, profile, string(langJSON), string(keysJSON), string(wordsJSON), string(packsJSON), string(tagsJSON)); err != nil {
		return nil, err
	}

	boosts := make([]model.WordBoost, 0, len(topWords))
	for _, w := range topWords {
		boost := scoreBoost(w.Count, w.AvgPos)
		boosts = append(boosts, model.WordBoost{
			Pinyin: w.Key,
			Word:   w.Word,
			Boost:  boost,
			Freq:   w.Count,
		})
	}
	sort.Slice(boosts, func(i, j int) bool { return boosts[i].Boost > boosts[j].Boost })
	if len(boosts) > 200 {
		boosts = boosts[:200]
	}
	ver := time.Now().UTC().Unix()
	if err := s.Store.ReplaceBoosts(ctx, deviceID, ver, boosts); err != nil {
		return nil, err
	}
	return &model.PersonalizationPack{
		DeviceID:  deviceID,
		Version:   ver,
		Generated: time.Now().UTC(),
		Boosts:    boosts,
		Tags:      profile.PersonaTags,
	}, nil
}

func (s *Services) GetProfile(ctx context.Context, deviceID string) (*model.UserProfile, error) {
	userID, langPrefs, topKeys, topWords, packs, tags, avgPos, selectCount, backRate, updated, err := s.Store.GetProfileRaw(ctx, deviceID)
	if err != nil {
		return nil, err
	}
	p := &model.UserProfile{
		DeviceID:       deviceID,
		UserID:         userID,
		AvgSelectPos:   avgPos,
		SelectCount:    selectCount,
		BackspaceRate:  backRate,
		UpdatedAt:      parseRFC(updated),
		LangPrefs:      map[string]int64{},
	}
	_ = json.Unmarshal([]byte(langPrefs), &p.LangPrefs)
	_ = json.Unmarshal([]byte(topKeys), &p.TopKeys)
	_ = json.Unmarshal([]byte(topWords), &p.TopWords)
	_ = json.Unmarshal([]byte(packs), &p.PreferredPackIDs)
	_ = json.Unmarshal([]byte(tags), &p.PersonaTags)
	return p, nil
}

func (s *Services) GetPersonalization(ctx context.Context, deviceID string) (*model.PersonalizationPack, error) {
	boosts, ver, err := s.Store.ListBoosts(ctx, deviceID)
	if err != nil {
		return nil, err
	}
	p, _ := s.GetProfile(ctx, deviceID)
	tags := []string{}
	if p != nil {
		tags = p.PersonaTags
	}
	return &model.PersonalizationPack{
		DeviceID:  deviceID,
		Version:   ver,
		Generated: time.Now().UTC(),
		Boosts:    boosts,
		Tags:      tags,
	}, nil
}

func (s *Services) RebuildAllActive(ctx context.Context) (int, error) {
	ids, err := s.Store.DeviceIDsWithHabits(ctx, time.Now().UTC().Add(-30*24*time.Hour))
	if err != nil {
		return 0, err
	}
	n := 0
	for _, id := range ids {
		if _, err := s.RebuildProfileAndBoosts(ctx, id); err != nil {
			return n, err
		}
		n++
	}
	return n, nil
}

func scoreBoost(count int64, avgPos float64) float64 {
	// Higher select count and later candidate position → stronger personalization need.
	posPenalty := avgPos
	if posPenalty < 0 {
		posPenalty = 0
	}
	return float64(count)*1.5 + posPenalty*2.0
}

func derivePersonaTags(avgPos, backRate float64, selects int64, langs map[string]int64) []string {
	tags := []string{}
	if selects < 20 {
		tags = append(tags, "new_user")
	} else if selects > 500 {
		tags = append(tags, "power_user")
	}
	if avgPos >= 3 {
		tags = append(tags, "needs_rerank")
	} else if avgPos <= 1.2 && selects > 50 {
		tags = append(tags, "high_precision")
	}
	if backRate >= 0.35 {
		tags = append(tags, "high_correction")
	}
	if len(langs) >= 2 {
		tags = append(tags, "multilingual")
	}
	for lang := range langs {
		tags = append(tags, "lang_"+lang)
	}
	return tags
}

func topNKeys(m map[string]int64, n int) []string {
	type kv struct {
		k string
		v int64
	}
	arr := make([]kv, 0, len(m))
	for k, v := range m {
		arr = append(arr, kv{k, v})
	}
	sort.Slice(arr, func(i, j int) bool { return arr[i].v > arr[j].v })
	if len(arr) > n {
		arr = arr[:n]
	}
	out := make([]string, 0, len(arr))
	for _, x := range arr {
		out = append(out, x.k)
	}
	return out
}

func parseRFC(s string) time.Time {
	t, err := time.Parse(time.RFC3339Nano, s)
	if err != nil {
		t, _ = time.Parse(time.RFC3339, s)
	}
	return t
}
