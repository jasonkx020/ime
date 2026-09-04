package service_test

import (
	"bytes"
	"context"
	"path/filepath"
	"testing"
	"time"

	"yc-admin/internal/model"
	"yc-admin/internal/service"
	"yc-admin/internal/store"
)

func TestLangPackPublishAndCatalog(t *testing.T) {
	dir := t.TempDir()
	st, err := store.Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()
	svc := &service.Services{Store: st, PublicBase: "http://example.test"}

	ctx := context.Background()
	p, err := svc.CreateDraft(ctx, model.LangPack{
		PackID: "zh-pack-v1", Lang: "zh", DisplayName: "中文", Version: 1,
	})
	if err != nil {
		t.Fatal(err)
	}
	payload := []byte("fake-imepack-bytes")
	p, err = svc.UploadArtifact(ctx, p.ID, bytes.NewReader(payload), "x.imepack")
	if err != nil {
		t.Fatal(err)
	}
	if p.Status != "review" || p.SHA256 == "" {
		t.Fatalf("unexpected after upload: %+v", p)
	}
	p, err = svc.Publish(ctx, p.ID)
	if err != nil {
		t.Fatal(err)
	}
	if p.Status != "published" {
		t.Fatalf("want published, got %s", p.Status)
	}
	path, err := svc.ArtifactPath(p.FileName)
	if err != nil {
		t.Fatal(err)
	}
	if filepath.Base(path) != p.FileName {
		t.Fatalf("bad path %s", path)
	}
	cat, err := svc.BuildCatalog(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if len(cat.Entries) != 1 || cat.Entries[0].PackID != "zh-pack-v1" {
		t.Fatalf("catalog: %+v", cat)
	}
}

func TestHabitProfileBoost(t *testing.T) {
	dir := t.TempDir()
	st, err := store.Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()
	svc := &service.Services{Store: st, PublicBase: "http://example.test"}
	ctx := context.Background()
	now := time.Now().UTC()
	n, err := svc.IngestHabits(ctx, []model.HabitEvent{
		{DeviceID: "d1", Lang: "zh", PackID: "zh-pack-v1", EventType: "select", QueryKey: "ta", SelectedWord: "他", CandidatePos: 4, PrivacyOK: true, OccurredAt: now},
		{DeviceID: "d1", Lang: "zh", PackID: "zh-pack-v1", EventType: "select", QueryKey: "ta", SelectedWord: "他", CandidatePos: 3, PrivacyOK: true, OccurredAt: now},
		{DeviceID: "d1", Lang: "zh", EventType: "backspace", QueryKey: "tai", PrivacyOK: true, OccurredAt: now},
		{DeviceID: "d1", Lang: "zh", EventType: "select", QueryKey: "wo", SelectedWord: "我", CandidatePos: 0, PrivacyOK: false, OccurredAt: now},
	})
	if err != nil {
		t.Fatal(err)
	}
	if n != 3 {
		t.Fatalf("accepted=%d", n)
	}
	prof, err := svc.GetProfile(ctx, "d1")
	if err != nil {
		t.Fatal(err)
	}
	if prof.SelectCount != 2 {
		t.Fatalf("select_count=%d", prof.SelectCount)
	}
	perso, err := svc.GetPersonalization(ctx, "d1")
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for _, b := range perso.Boosts {
		if b.Word == "他" && b.Pinyin == "ta" && b.Boost > 0 {
			found = true
		}
	}
	if !found {
		t.Fatalf("missing boost for 他: %+v", perso.Boosts)
	}
}
