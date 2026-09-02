#pragma once

#include <cstdint>
#include <functional>
#include <string>

#include "yc_arena_parser.hpp"
#include "yc_hot.h"

namespace yc {

using CommitHandler = std::function<void(const std::string &)>;

class HotPathClient {
public:
    uint64_t editor_id_{0};
    uint64_t client_seq_{0};
    uint64_t last_seq_{0};

    int init(const char *data_dir) { return yc_core_init(data_dir); }

    void shutdown() { yc_core_shutdown(); }

    uint64_t beginSession(uint64_t field_id, uint32_t input_type = 0) {
        editor_id_ = yc_session_begin_with_input(field_id, input_type);
        client_seq_ = 0;
        last_seq_ = 0;
        if (editor_id_ != 0) {
            submitInit();
        }
        return editor_id_;
    }

    void stopSession(uint32_t reason = 0) {
        if (editor_id_ != 0) {
            yc_session_stop(editor_id_, reason);
            editor_id_ = 0;
        }
    }

    bool validate() const {
        return editor_id_ != 0 && yc_session_validate(editor_id_) != 0;
    }

    int submitKeyPress(uint32_t key_code) {
        return submitAction(YC_ACTION_KEY_PRESS, key_code, 0);
    }

    int submitSelectCandidate(uint32_t candidate_id) {
        return submitAction(YC_ACTION_SELECT_CANDIDATE, 0, candidate_id);
    }

    int submitBackspace() { return submitAction(YC_ACTION_BACKSPACE, 0, 0); }

    arena::Snapshot readArena() const {
        const uint8_t *ptr = yc_hot_arena_ptr();
        const size_t size = yc_hot_arena_size();
        if (ptr == nullptr || size == 0) {
            return {};
        }
        return arena::parse(ptr, size);
    }

    void refreshUi(const CommitHandler &on_commit) {
        const auto snap = readArena();
        if (snap.seq == last_seq_ || snap.editor_id != editor_id_) {
            return;
        }
        last_seq_ = snap.seq;
        for (const auto &cmd : snap.commands) {
            switch (cmd.type) {
            case arena::CommandType::Commit:
                if (on_commit) {
                    on_commit(cmd.text);
                }
                break;
            case arena::CommandType::FinishComposing:
                break;
            default:
                break;
            }
        }
    }

private:
    int submitInit() { return submitAction(YC_ACTION_INIT, 0, 0); }

    int submitAction(uint32_t action_type, uint32_t key_code, uint32_t candidate_id) {
        if (editor_id_ == 0) {
            return YC_ERR_SESSION;
        }
        ++client_seq_;
        YcHotAction action{};
        action.editor_id = editor_id_;
        action.client_seq = client_seq_;
        action.action_type = action_type;
        action.key_code = key_code;
        action.candidate_id = candidate_id;
        return yc_hot_submit(&action);
    }
};

} // namespace yc
