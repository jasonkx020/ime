#pragma once

#include <functional>
#include <string>

#include "yc_theme_tokens.hpp"

namespace yc::ui {

using KeyHandler = std::function<void(uint32_t key_code)>;
using CandidateHandler = std::function<void(uint32_t candidate_id)>;

class IKeyboardPanel {
public:
    virtual ~IKeyboardPanel() = default;
    virtual void apply_theme(const ThemeTokens &tokens) = 0;
    virtual void render(const KeyboardSnapshot &snapshot) = 0;
    virtual void set_key_handler(KeyHandler handler) = 0;
    virtual void set_candidate_handler(CandidateHandler handler) = 0;
};

} // namespace yc::ui
