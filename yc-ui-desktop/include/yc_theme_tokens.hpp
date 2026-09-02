#pragma once

#include <cstdint>

namespace yc::ui {

struct ThemeTokens {
    uint32_t keyboard_bg = 0xFFE8EAED;
    uint32_t key_normal = 0xFFFFFFFF;
    uint32_t key_utility = 0xFFDDE0E4;
    uint32_t key_accent = 0xFF1A73E8;
    uint32_t key_pressed = 0xFFC8CCD2;
    uint32_t cand_text = 0xFF202124;
    uint32_t composing_text = 0xFF1A73E8;
    float key_radius = 12.f;
};

struct CandidateItem {
    uint32_t id{};
    const char *text{};
};

struct KeyboardSnapshot {
    uint64_t editor_id{};
    uint64_t seq{};
    const char *composing{};
    const CandidateItem *candidates{};
    size_t candidate_count{};
};

enum class KeyStyle { Normal, Utility, Accent };

struct KeyDef {
    const char *label;
    float width_weight = 1.f;
    KeyStyle style = KeyStyle::Normal;
    uint32_t key_code = 0;
};

} // namespace yc::ui
