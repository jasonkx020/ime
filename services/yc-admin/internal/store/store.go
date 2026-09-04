package store

import (
	"context"
	"database/sql"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"yc-admin/internal/model"

	_ "modernc.org/sqlite"
)

type Store struct {
	db      *sql.DB
	dataDir string
}

func Open(dataDir string) (*Store, error) {
	if err := os.MkdirAll(filepath.Join(dataDir, "packs"), 0o755); err != nil {
		return nil, err
	}
	dbPath := filepath.Join(dataDir, "yc-admin.db")
	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(1)
	s := &Store{db: db, dataDir: dataDir}
	if err := s.migrate(); err != nil {
		_ = db.Close()
		return nil, err
	}
	return s, nil
}

func (s *Store) Close() error { return s.db.Close() }

func (s *Store) DataDir() string { return s.dataDir }

func (s *Store) PacksDir() string { return filepath.Join(s.dataDir, "packs") }

func (s *Store) migrate() error {
	_, err := s.db.Exec(`
CREATE TABLE IF NOT EXISTS lang_packs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  pack_id TEXT NOT NULL,
  lang TEXT NOT NULL,
  display_name TEXT NOT NULL,
  version INTEGER NOT NULL,
  min_host_version TEXT NOT NULL DEFAULT '0.1.0',
  status TEXT NOT NULL DEFAULT 'draft',
  sha256 TEXT NOT NULL DEFAULT '',
  size_bytes INTEGER NOT NULL DEFAULT 0,
  file_name TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  published_at TEXT,
  UNIQUE(pack_id, version)
);
CREATE INDEX IF NOT EXISTS idx_lang_packs_status ON lang_packs(status);

CREATE TABLE IF NOT EXISTS habit_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  device_id TEXT NOT NULL,
  user_id TEXT NOT NULL DEFAULT '',
  lang TEXT NOT NULL DEFAULT '',
  pack_id TEXT NOT NULL DEFAULT '',
  event_type TEXT NOT NULL,
  query_key TEXT NOT NULL DEFAULT '',
  selected_word TEXT NOT NULL DEFAULT '',
  candidate_pos INTEGER NOT NULL DEFAULT 0,
  privacy_ok INTEGER NOT NULL DEFAULT 0,
  occurred_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_habit_device_time ON habit_events(device_id, occurred_at);
CREATE INDEX IF NOT EXISTS idx_habit_select ON habit_events(device_id, event_type, selected_word);

CREATE TABLE IF NOT EXISTS user_profiles (
  device_id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL DEFAULT '',
  lang_prefs_json TEXT NOT NULL DEFAULT '{}',
  top_keys_json TEXT NOT NULL DEFAULT '[]',
  top_words_json TEXT NOT NULL DEFAULT '[]',
  avg_select_pos REAL NOT NULL DEFAULT 0,
  select_count INTEGER NOT NULL DEFAULT 0,
  backspace_rate REAL NOT NULL DEFAULT 0,
  preferred_packs_json TEXT NOT NULL DEFAULT '[]',
  persona_tags_json TEXT NOT NULL DEFAULT '[]',
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS word_boosts (
  device_id TEXT NOT NULL,
  pinyin TEXT NOT NULL,
  word TEXT NOT NULL,
  boost REAL NOT NULL,
  freq INTEGER NOT NULL,
  version INTEGER NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(device_id, pinyin, word)
);
CREATE INDEX IF NOT EXISTS idx_boost_device ON word_boosts(device_id);

CREATE TABLE IF NOT EXISTS catalog_meta (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  catalog_version INTEGER NOT NULL DEFAULT 1
);
INSERT OR IGNORE INTO catalog_meta(id, catalog_version) VALUES (1, 1);
`)
	return err
}

func nowUTC() time.Time { return time.Now().UTC() }

func fmtTime(t time.Time) string { return t.UTC().Format(time.RFC3339Nano) }

func parseTime(s string) time.Time {
	t, err := time.Parse(time.RFC3339Nano, s)
	if err != nil {
		t, _ = time.Parse(time.RFC3339, s)
	}
	return t
}

func (s *Store) CreateLangPack(ctx context.Context, p *model.LangPack) error {
	now := nowUTC()
	p.CreatedAt = now
	p.UpdatedAt = now
	if p.Status == "" {
		p.Status = "draft"
	}
	res, err := s.db.ExecContext(ctx, `
INSERT INTO lang_packs(pack_id, lang, display_name, version, min_host_version, status, sha256, size_bytes, file_name, notes, created_at, updated_at, published_at)
VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)`,
		p.PackID, p.Lang, p.DisplayName, p.Version, p.MinHostVersion, p.Status,
		p.SHA256, p.SizeBytes, p.FileName, p.Notes, fmtTime(p.CreatedAt), fmtTime(p.UpdatedAt), nil)
	if err != nil {
		return err
	}
	id, _ := res.LastInsertId()
	p.ID = id
	return nil
}

func (s *Store) UpdateLangPack(ctx context.Context, p *model.LangPack) error {
	p.UpdatedAt = nowUTC()
	var published any
	if p.PublishedAt != nil {
		published = fmtTime(*p.PublishedAt)
	}
	_, err := s.db.ExecContext(ctx, `
UPDATE lang_packs SET lang=?, display_name=?, min_host_version=?, status=?, sha256=?, size_bytes=?, file_name=?, notes=?, updated_at=?, published_at=?
WHERE id=?`,
		p.Lang, p.DisplayName, p.MinHostVersion, p.Status, p.SHA256, p.SizeBytes, p.FileName, p.Notes,
		fmtTime(p.UpdatedAt), published, p.ID)
	return err
}

func (s *Store) GetLangPack(ctx context.Context, id int64) (*model.LangPack, error) {
	row := s.db.QueryRowContext(ctx, `
SELECT id, pack_id, lang, display_name, version, min_host_version, status, sha256, size_bytes, file_name, notes, created_at, updated_at, published_at
FROM lang_packs WHERE id=?`, id)
	return scanLangPack(row)
}

func (s *Store) GetLangPackByPackVersion(ctx context.Context, packID string, version uint32) (*model.LangPack, error) {
	row := s.db.QueryRowContext(ctx, `
SELECT id, pack_id, lang, display_name, version, min_host_version, status, sha256, size_bytes, file_name, notes, created_at, updated_at, published_at
FROM lang_packs WHERE pack_id=? AND version=?`, packID, version)
	return scanLangPack(row)
}

func (s *Store) ListLangPacks(ctx context.Context, status string) ([]model.LangPack, error) {
	q := `
SELECT id, pack_id, lang, display_name, version, min_host_version, status, sha256, size_bytes, file_name, notes, created_at, updated_at, published_at
FROM lang_packs`
	args := []any{}
	if status != "" {
		q += ` WHERE status=?`
		args = append(args, status)
	}
	q += ` ORDER BY updated_at DESC`
	rows, err := s.db.QueryContext(ctx, q, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []model.LangPack
	for rows.Next() {
		p, err := scanLangPackRows(rows)
		if err != nil {
			return nil, err
		}
		out = append(out, *p)
	}
	return out, rows.Err()
}

func (s *Store) ListPublishedLatest(ctx context.Context) ([]model.LangPack, error) {
	rows, err := s.db.QueryContext(ctx, `
SELECT id, pack_id, lang, display_name, version, min_host_version, status, sha256, size_bytes, file_name, notes, created_at, updated_at, published_at
FROM lang_packs p
WHERE status='published'
  AND version = (
    SELECT MAX(version) FROM lang_packs p2 WHERE p2.pack_id=p.pack_id AND p2.status='published'
  )
ORDER BY pack_id`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []model.LangPack
	for rows.Next() {
		p, err := scanLangPackRows(rows)
		if err != nil {
			return nil, err
		}
		out = append(out, *p)
	}
	return out, rows.Err()
}

func (s *Store) BumpCatalogVersion(ctx context.Context) (uint32, error) {
	_, err := s.db.ExecContext(ctx, `UPDATE catalog_meta SET catalog_version = catalog_version + 1 WHERE id=1`)
	if err != nil {
		return 0, err
	}
	var v uint32
	err = s.db.QueryRowContext(ctx, `SELECT catalog_version FROM catalog_meta WHERE id=1`).Scan(&v)
	return v, err
}

func (s *Store) CatalogVersion(ctx context.Context) (uint32, error) {
	var v uint32
	err := s.db.QueryRowContext(ctx, `SELECT catalog_version FROM catalog_meta WHERE id=1`).Scan(&v)
	return v, err
}

type scannable interface {
	Scan(dest ...any) error
}

func scanLangPack(row scannable) (*model.LangPack, error) {
	var p model.LangPack
	var created, updated string
	var published sql.NullString
	err := row.Scan(&p.ID, &p.PackID, &p.Lang, &p.DisplayName, &p.Version, &p.MinHostVersion, &p.Status,
		&p.SHA256, &p.SizeBytes, &p.FileName, &p.Notes, &created, &updated, &published)
	if err != nil {
		return nil, err
	}
	p.CreatedAt = parseTime(created)
	p.UpdatedAt = parseTime(updated)
	if published.Valid && published.String != "" {
		t := parseTime(published.String)
		p.PublishedAt = &t
	}
	return &p, nil
}

func scanLangPackRows(rows *sql.Rows) (*model.LangPack, error) {
	return scanLangPack(rows)
}

func (s *Store) InsertHabitEvents(ctx context.Context, events []model.HabitEvent) error {
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()
	stmt, err := tx.PrepareContext(ctx, `
INSERT INTO habit_events(device_id, user_id, lang, pack_id, event_type, query_key, selected_word, candidate_pos, privacy_ok, occurred_at)
VALUES(?,?,?,?,?,?,?,?,?,?)`)
	if err != nil {
		return err
	}
	defer stmt.Close()
	for _, e := range events {
		if !e.PrivacyOK {
			continue
		}
		if e.OccurredAt.IsZero() {
			e.OccurredAt = nowUTC()
		}
		privacy := 0
		if e.PrivacyOK {
			privacy = 1
		}
		if _, err := stmt.ExecContext(ctx, e.DeviceID, e.UserID, e.Lang, e.PackID, e.EventType,
			strings.ToLower(strings.TrimSpace(e.QueryKey)), e.SelectedWord, e.CandidatePos, privacy, fmtTime(e.OccurredAt)); err != nil {
			return err
		}
	}
	return tx.Commit()
}

func (s *Store) UpsertProfile(ctx context.Context, p *model.UserProfile, langPrefs, topKeys, topWords, packs, tags string) error {
	p.UpdatedAt = nowUTC()
	_, err := s.db.ExecContext(ctx, `
INSERT INTO user_profiles(device_id, user_id, lang_prefs_json, top_keys_json, top_words_json, avg_select_pos, select_count, backspace_rate, preferred_packs_json, persona_tags_json, updated_at)
VALUES(?,?,?,?,?,?,?,?,?,?,?)
ON CONFLICT(device_id) DO UPDATE SET
  user_id=excluded.user_id,
  lang_prefs_json=excluded.lang_prefs_json,
  top_keys_json=excluded.top_keys_json,
  top_words_json=excluded.top_words_json,
  avg_select_pos=excluded.avg_select_pos,
  select_count=excluded.select_count,
  backspace_rate=excluded.backspace_rate,
  preferred_packs_json=excluded.preferred_packs_json,
  persona_tags_json=excluded.persona_tags_json,
  updated_at=excluded.updated_at`,
		p.DeviceID, p.UserID, langPrefs, topKeys, topWords, p.AvgSelectPos, p.SelectCount, p.BackspaceRate, packs, tags, fmtTime(p.UpdatedAt))
	return err
}

func (s *Store) GetProfileRaw(ctx context.Context, deviceID string) (userID, langPrefs, topKeys, topWords, packs, tags string, avgPos float64, selectCount int64, backspaceRate float64, updated string, err error) {
	err = s.db.QueryRowContext(ctx, `
SELECT user_id, lang_prefs_json, top_keys_json, top_words_json, avg_select_pos, select_count, backspace_rate, preferred_packs_json, persona_tags_json, updated_at
FROM user_profiles WHERE device_id=?`, deviceID).Scan(
		&userID, &langPrefs, &topKeys, &topWords, &avgPos, &selectCount, &backspaceRate, &packs, &tags, &updated)
	return
}

func (s *Store) ListProfileIDs(ctx context.Context, limit int) ([]string, error) {
	if limit <= 0 {
		limit = 100
	}
	rows, err := s.db.QueryContext(ctx, `SELECT device_id FROM user_profiles ORDER BY updated_at DESC LIMIT ?`, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var ids []string
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			return nil, err
		}
		ids = append(ids, id)
	}
	return ids, rows.Err()
}

type AggregateRow struct {
	Key   string
	Word  string
	Count int64
	AvgPos float64
}

func (s *Store) AggregateDeviceHabits(ctx context.Context, deviceID string, since time.Time) (selects, backspaces int64, langs map[string]int64, packs map[string]int64, topKeys, topWords []AggregateRow, avgPos float64, err error) {
	langs = map[string]int64{}
	packs = map[string]int64{}
	sinceStr := fmtTime(since)

	_ = s.db.QueryRowContext(ctx, `
SELECT COUNT(*) FROM habit_events WHERE device_id=? AND privacy_ok=1 AND event_type='select' AND occurred_at>=?`, deviceID, sinceStr).Scan(&selects)
	_ = s.db.QueryRowContext(ctx, `
SELECT COUNT(*) FROM habit_events WHERE device_id=? AND privacy_ok=1 AND event_type='backspace' AND occurred_at>=?`, deviceID, sinceStr).Scan(&backspaces)
	_ = s.db.QueryRowContext(ctx, `
SELECT COALESCE(AVG(candidate_pos),0) FROM habit_events WHERE device_id=? AND privacy_ok=1 AND event_type='select' AND occurred_at>=?`, deviceID, sinceStr).Scan(&avgPos)

	rows, err := s.db.QueryContext(ctx, `
SELECT lang, COUNT(*) FROM habit_events WHERE device_id=? AND privacy_ok=1 AND occurred_at>=? AND lang!='' GROUP BY lang`, deviceID, sinceStr)
	if err != nil {
		return
	}
	for rows.Next() {
		var k string
		var c int64
		_ = rows.Scan(&k, &c)
		langs[k] = c
	}
	rows.Close()

	rows, err = s.db.QueryContext(ctx, `
SELECT pack_id, COUNT(*) FROM habit_events WHERE device_id=? AND privacy_ok=1 AND occurred_at>=? AND pack_id!='' GROUP BY pack_id`, deviceID, sinceStr)
	if err != nil {
		return
	}
	for rows.Next() {
		var k string
		var c int64
		_ = rows.Scan(&k, &c)
		packs[k] = c
	}
	rows.Close()

	rows, err = s.db.QueryContext(ctx, `
SELECT query_key, COUNT(*) AS c FROM habit_events
WHERE device_id=? AND privacy_ok=1 AND event_type='select' AND occurred_at>=? AND query_key!=''
GROUP BY query_key ORDER BY c DESC LIMIT 30`, deviceID, sinceStr)
	if err != nil {
		return
	}
	for rows.Next() {
		var r AggregateRow
		_ = rows.Scan(&r.Key, &r.Count)
		topKeys = append(topKeys, r)
	}
	rows.Close()

	rows, err = s.db.QueryContext(ctx, `
SELECT selected_word, query_key, COUNT(*) AS c, AVG(candidate_pos)
FROM habit_events
WHERE device_id=? AND privacy_ok=1 AND event_type='select' AND occurred_at>=? AND selected_word!=''
GROUP BY selected_word, query_key ORDER BY c DESC LIMIT 50`, deviceID, sinceStr)
	if err != nil {
		return
	}
	for rows.Next() {
		var r AggregateRow
		_ = rows.Scan(&r.Word, &r.Key, &r.Count, &r.AvgPos)
		topWords = append(topWords, r)
	}
	rows.Close()
	return
}

func (s *Store) ReplaceBoosts(ctx context.Context, deviceID string, version int64, boosts []model.WordBoost) error {
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()
	if _, err := tx.ExecContext(ctx, `DELETE FROM word_boosts WHERE device_id=?`, deviceID); err != nil {
		return err
	}
	stmt, err := tx.PrepareContext(ctx, `
INSERT INTO word_boosts(device_id, pinyin, word, boost, freq, version, updated_at) VALUES(?,?,?,?,?,?,?)`)
	if err != nil {
		return err
	}
	defer stmt.Close()
	now := fmtTime(nowUTC())
	for _, b := range boosts {
		if _, err := stmt.ExecContext(ctx, deviceID, b.Pinyin, b.Word, b.Boost, b.Freq, version, now); err != nil {
			return err
		}
	}
	return tx.Commit()
}

func (s *Store) ListBoosts(ctx context.Context, deviceID string) ([]model.WordBoost, int64, error) {
	rows, err := s.db.QueryContext(ctx, `
SELECT pinyin, word, boost, freq, version FROM word_boosts WHERE device_id=? ORDER BY boost DESC`, deviceID)
	if err != nil {
		return nil, 0, err
	}
	defer rows.Close()
	var out []model.WordBoost
	var ver int64
	for rows.Next() {
		var b model.WordBoost
		var v int64
		if err := rows.Scan(&b.Pinyin, &b.Word, &b.Boost, &b.Freq, &v); err != nil {
			return nil, 0, err
		}
		ver = v
		out = append(out, b)
	}
	return out, ver, rows.Err()
}

func (s *Store) Dashboard(ctx context.Context) (model.DashboardStats, error) {
	var st model.DashboardStats
	_ = s.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM lang_packs WHERE status='published'`).Scan(&st.PublishedPacks)
	_ = s.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM lang_packs WHERE status='draft'`).Scan(&st.DraftPacks)
	_ = s.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM user_profiles`).Scan(&st.Profiles)
	since := fmtTime(nowUTC().Add(-7 * 24 * time.Hour))
	_ = s.db.QueryRowContext(ctx, `SELECT COUNT(DISTINCT device_id) FROM habit_events WHERE occurred_at>=? AND privacy_ok=1`, since).Scan(&st.ActiveDevices)
	_ = s.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM habit_events WHERE event_type='select' AND occurred_at>=? AND privacy_ok=1`, since).Scan(&st.SelectEvents7d)
	return st, nil
}

func (s *Store) DeviceIDsWithHabits(ctx context.Context, since time.Time) ([]string, error) {
	rows, err := s.db.QueryContext(ctx, `
SELECT DISTINCT device_id FROM habit_events WHERE privacy_ok=1 AND occurred_at>=?`, fmtTime(since))
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var ids []string
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			return nil, err
		}
		ids = append(ids, id)
	}
	return ids, rows.Err()
}

func ErrNotFound(entity string) error {
	return fmt.Errorf("%s not found", entity)
}
