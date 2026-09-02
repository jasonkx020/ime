#pragma once

#include <cstdint>
#include <fstream>
#include <string>
#include <vector>

#include "yc_layout.h"

namespace yc::layout {

struct KeyDef {
    std::string label;
    std::string output;
    uint8_t action{};
    float width{1.f};
};

inline std::vector<KeyDef> load_from_pack(const std::string &data_dir,
                                          const std::string &layout_id) {
    std::vector<KeyDef> keys;
    const std::string path =
        data_dir + "/langpacks/* is resolved by caller"; // placeholder
    (void)path;
    return keys;
}

inline std::vector<KeyDef> parse_bin(const std::string &bin_path) {
    std::vector<KeyDef> keys;
    std::ifstream in(bin_path, std::ios::binary);
    if (!in) {
        return keys;
    }
    YcLayoutHeader header{};
    in.read(reinterpret_cast<char *>(&header), sizeof(YcLayoutHeader));
    if (std::string(header.magic, 4) != YC_LAYOUT_MAGIC) {
        return keys;
    }
    for (uint32_t i = 0; i < header.key_count; ++i) {
        YcKeySlot slot{};
        in.read(reinterpret_cast<char *>(&slot), sizeof(YcKeySlot));
        KeyDef k;
        k.label = slot.label;
        k.output = slot.output;
        k.action = slot.action;
        k.width = slot.width;
        keys.push_back(std::move(k));
    }
    return keys;
}

} // namespace yc::layout
