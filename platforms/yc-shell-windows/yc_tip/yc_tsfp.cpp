#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include "yc_platform_adapter.h"

namespace {

void yc_tip_log(const char *msg) { OutputDebugStringA(msg); }

} // namespace

BOOL WINAPI DllMain(HINSTANCE instance, DWORD reason, LPVOID reserved) {
    (void)instance;
    (void)reserved;

    switch (reason) {
    case DLL_PROCESS_ATTACH:
        DisableThreadLibraryCalls(instance);
        yc_tip_log("[yc_tip] M0 smoke rc=");
        {
            char buf[64];
            wsprintfA(buf, "%d\n", yc_platform_smoke(nullptr));
            yc_tip_log(buf);
        }
        yc_tip_log("[yc_tip] M1 smoke rc=");
        {
            char buf[64];
            wsprintfA(buf, "%d commit=%s\n", yc_platform_m1_smoke(nullptr),
                      yc_platform_last_commit());
            yc_tip_log(buf);
        }
        break;
    case DLL_PROCESS_DETACH:
        break;
    default:
        break;
    }
    return TRUE;
}

extern "C" __declspec(dllexport) void yc_tsfp_stub(void) {}