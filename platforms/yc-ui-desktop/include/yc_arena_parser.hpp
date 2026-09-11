#pragma once

#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

#include "yc_hot.h"

namespace yc::arena {

struct Candidate {
    uint32_t id{};
    std::string text;
};

enum class CommandType : uint32_t {
    Commit = YC_CMD_COMMIT,
    SetComposing = YC_CMD_SET_COMPOSING,
    FinishComposing = YC_CMD_FINISH_COMPOSING,
    DeleteSurrounding = YC_CMD_DELETE_SURROUNDING,
    ReloadKeyboard = YC_CMD_RELOAD_KEYBOARD,
};

struct Command {
    CommandType type{CommandType::Commit};
    std::string text;
    uint32_t param0{};
    uint32_t param1{};
};

struct Snapshot {
    uint64_t editor_id{};
    uint64_t seq{};
    uint32_t status_flags{};
    std::string composing;
    std::vector<Candidate> candidates;
    std::vector<Command> commands;
};

inline Snapshot parse(const uint8_t *data, size_t len) {
    Snapshot out;
    if (data == nullptr || len < sizeof(YcHotHeader)) {
        return out;
    }

    const auto *header = reinterpret_cast<const YcHotHeader *>(data);
    out.editor_id = header->editor_id;
    out.seq = header->seq;
    out.status_flags = header->status_flags;

    const size_t composing_off = sizeof(YcHotHeader);
    const size_t composing_len = header->composing_len;
    if (composing_off + composing_len <= len) {
        out.composing.assign(reinterpret_cast<const char *>(data + composing_off), composing_len);
    }

    const size_t slots_off = composing_off + YC_MAX_COMPOSING_LEN;
    const size_t slot_size = sizeof(YcCandidateSlot);
    for (uint32_t i = 0; i < header->cand_count && i < YC_MAX_CANDIDATES; ++i) {
        const size_t off = slots_off + i * slot_size;
        if (off + slot_size > len) {
            break;
        }
        const auto *slot = reinterpret_cast<const YcCandidateSlot *>(data + off);
        Candidate c;
        c.id = slot->id;
        const size_t tlen = slot->text_len;
        if (tlen > 0 && tlen <= YC_MAX_CAND_TEXT_LEN) {
            c.text.assign(reinterpret_cast<const char *>(slot->text), tlen);
        }
        out.candidates.push_back(std::move(c));
    }

    const size_t cmds_off = slots_off + YC_MAX_CANDIDATES * slot_size;
    const size_t cmd_slot_size = sizeof(YcUiCommandSlot);
    for (uint32_t i = 0; i < header->cmd_count && i < YC_MAX_ARENA_COMMANDS; ++i) {
        const size_t off = cmds_off + i * cmd_slot_size;
        if (off + cmd_slot_size > len) {
            break;
        }
        const auto *slot = reinterpret_cast<const YcUiCommandSlot *>(data + off);
        Command cmd;
        cmd.type = static_cast<CommandType>(slot->cmd_type);
        cmd.param0 = slot->param0;
        cmd.param1 = slot->param1;
        if (slot->text_len > 0 && slot->text_len <= YC_MAX_CAND_TEXT_LEN) {
            cmd.text.assign(reinterpret_cast<const char *>(slot->text), slot->text_len);
        }
        out.commands.push_back(std::move(cmd));
    }

    return out;
}

} // namespace yc::arena
