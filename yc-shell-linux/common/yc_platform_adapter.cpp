#include "yc_platform_adapter.h"

#include "yc_hot_path_client.hpp"
#include "yc_hot.h"

#include <cstdio>

static yc::HotPathClient g_client;
static std::string g_last_commit;

int yc_platform_smoke(const char *data_dir) {
    const char *dir = (data_dir != nullptr && data_dir[0] != '\0') ? data_dir : ".";

    if (g_client.init(dir) != YC_OK) {
        return -1;
    }

    const uint64_t editor_id = g_client.beginSession(1);
    if (editor_id == 0) {
        g_client.shutdown();
        return -2;
    }

    if (!g_client.validate()) {
        g_client.stopSession(0);
        g_client.shutdown();
        return -3;
    }

    g_client.stopSession(0);
    g_client.shutdown();
    return 0;
}

int yc_platform_m1_smoke(const char *data_dir) {
    const char *dir = (data_dir != nullptr && data_dir[0] != '\0') ? data_dir : ".";
    g_last_commit.clear();

    if (g_client.init(dir) != YC_OK) {
        return -1;
    }
    if (g_client.beginSession(1) == 0) {
        g_client.shutdown();
        return -2;
    }

    const char *word = "nihao";
    for (const char *p = word; *p; ++p) {
        if (g_client.submitKeyPress(static_cast<uint32_t>(*p)) != YC_OK) {
            g_client.stopSession(0);
            g_client.shutdown();
            return -3;
        }
        g_client.refreshUi([&](const std::string &text) { g_last_commit = text; });
    }

    const auto snap = g_client.readArena();
    if (!snap.candidates.empty()) {
        g_client.submitSelectCandidate(snap.candidates[0].id);
        g_client.refreshUi([&](const std::string &text) { g_last_commit = text; });
    }

    const int rc = g_last_commit.empty() ? -4 : 0;
    g_client.stopSession(0);
    g_client.shutdown();
    return rc;
}

const char *yc_platform_last_commit(void) { return g_last_commit.c_str(); }

yc::HotPathClient *yc_platform_client(void) { return &g_client; }
