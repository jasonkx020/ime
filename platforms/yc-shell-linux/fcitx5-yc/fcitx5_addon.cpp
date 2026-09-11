// M0 Fcitx5 addon stub — runs FFI smoke on plugin load.
// M1+ will implement fcitx5::InputMethodEngineV3 and register via
// FCITX_ADDON_FACTORY(fcitx5::AddonFactory).

#include "yc_platform_adapter.h"

#include <cstdio>

#ifndef YC_FCITX5_STUB
#include <fcitx/addonfactory.h>
#include <fcitx/addonmanager.h>
#endif

namespace {

__attribute__((constructor)) void yc_fcitx5_on_load() {
    const int rc = yc_platform_smoke(".");
    std::fprintf(stderr, "[fcitx5-yc] M0 smoke rc=%d\n", rc);
    const int m1 = yc_platform_m1_smoke(".");
    std::fprintf(stderr, "[fcitx5-yc] M1 smoke rc=%d commit=%s\n", m1, yc_platform_last_commit());
}

} // namespace

#ifndef YC_FCITX5_STUB

// M1+: fcitx5::AddonInstance subclass and FCITX_ADDON_FACTORY registration.

#endif /* !YC_FCITX5_STUB */
