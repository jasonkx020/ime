package model

import "time"

// LangPack lifecycle: draft → review → published → archived
type LangPack struct {
	ID             int64     `json:"id"`
	PackID         string    `json:"pack_id"`
	Lang           string    `json:"lang"`
	DisplayName    string    `json:"display_name"`
	Version        uint32    `json:"version"`
	MinHostVersion string    `json:"min_host_version"`
	Status         string    `json:"status"`
	SHA256         string    `json:"sha256,omitempty"`
	SizeBytes      int64     `json:"size_bytes"`
	FileName       string    `json:"file_name,omitempty"`
	Notes          string    `json:"notes,omitempty"`
	CreatedAt      time.Time `json:"created_at"`
	UpdatedAt      time.Time `json:"updated_at"`
	PublishedAt    *time.Time `json:"published_at,omitempty"`
}

type CatalogEntry struct {
	PackID         string `json:"pack_id"`
	Lang           string `json:"lang"`
	Version        uint32 `json:"version"`
	URL            string `json:"url"`
	SHA256         string `json:"sha256"`
	SizeBytes      uint64 `json:"size_bytes"`
	MinHostVersion string `json:"min_host_version"`
	DisplayName    string `json:"display_name,omitempty"`
}

type Catalog struct {
	CatalogVersion uint32         `json:"catalog_version"`
	FetchedAt      int64          `json:"fetched_at"`
	Entries        []CatalogEntry `json:"entries"`
}

// HabitEvent is a privacy-scoped telemetry event from the IME client.
// Prefer romanized key + selected word; avoid raw composing / app text.
type HabitEvent struct {
	DeviceID     string    `json:"device_id"`
	UserID       string    `json:"user_id,omitempty"`
	Lang         string    `json:"lang"`
	PackID       string    `json:"pack_id,omitempty"`
	EventType    string    `json:"event_type"` // select | commit | backspace | switch_scheme
	QueryKey     string    `json:"query_key"`
	SelectedWord string    `json:"selected_word,omitempty"`
	CandidatePos int       `json:"candidate_pos,omitempty"`
	PrivacyOK    bool      `json:"privacy_ok"`
	OccurredAt   time.Time `json:"occurred_at"`
}

type UserProfile struct {
	DeviceID          string            `json:"device_id"`
	UserID            string            `json:"user_id,omitempty"`
	LangPrefs         map[string]int64  `json:"lang_prefs"`
	TopKeys           []WordStat        `json:"top_keys"`
	TopWords          []WordStat        `json:"top_words"`
	AvgSelectPos      float64           `json:"avg_select_pos"`
	SelectCount       int64             `json:"select_count"`
	BackspaceRate     float64           `json:"backspace_rate"`
	PreferredPackIDs  []string          `json:"preferred_pack_ids"`
	PersonaTags       []string          `json:"persona_tags"`
	UpdatedAt         time.Time         `json:"updated_at"`
}

type WordStat struct {
	Key   string `json:"key,omitempty"`
	Word  string `json:"word"`
	Count int64  `json:"count"`
	Score float64 `json:"score,omitempty"`
}

// WordBoost is a personalization delta for client LightIntel / user_words.
type WordBoost struct {
	Pinyin string  `json:"pinyin"`
	Word   string  `json:"word"`
	Boost  float64 `json:"boost"`
	Freq   int64   `json:"freq"`
}

type PersonalizationPack struct {
	DeviceID  string      `json:"device_id"`
	Version   int64       `json:"version"`
	Generated time.Time   `json:"generated_at"`
	Boosts    []WordBoost `json:"boosts"`
	Tags      []string    `json:"persona_tags,omitempty"`
}

type DashboardStats struct {
	PublishedPacks int64 `json:"published_packs"`
	DraftPacks     int64 `json:"draft_packs"`
	ActiveDevices  int64 `json:"active_devices_7d"`
	SelectEvents7d int64 `json:"select_events_7d"`
	Profiles       int64 `json:"profiles"`
}
