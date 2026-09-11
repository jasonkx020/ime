#pragma once

#ifdef __cplusplus
extern "C" {
#endif

int yc_platform_smoke(const char *data_dir);
int yc_platform_m1_smoke(const char *data_dir);
const char *yc_platform_last_commit(void);

#ifdef __cplusplus
}

#include "yc_hot_path_client.hpp"

namespace yc {
yc::HotPathClient *yc_platform_client();
}

#endif
