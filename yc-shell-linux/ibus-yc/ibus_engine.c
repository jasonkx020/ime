/* M0 IBus engine stub — runs FFI smoke on plugin load.
 * M1+ will subclass IBusEngine and register via ibus_component_add_engine. */

#include "yc_platform_adapter.h"

#ifndef YC_IBUS_STUB
#include <ibus.h>
#endif

#include <stdio.h>

__attribute__((constructor)) static void yc_ibus_on_load(void) {
    const int rc = yc_platform_smoke(".");
    fprintf(stderr, "[ibus-yc] M0 smoke rc=%d\n", rc);
    const int m1 = yc_platform_m1_smoke(".");
    fprintf(stderr, "[ibus-yc] M1 smoke rc=%d commit=%s\n", m1, yc_platform_last_commit());
}

#ifndef YC_IBUS_STUB

/* M1+: implement IBusEngineClass and register yc IME engine here. */

#endif /* !YC_IBUS_STUB */
