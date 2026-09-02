#include "napi/native_api.h"
#include "yc_hot.h"

#include <cstring>
#include <vector>

static napi_value MakeInt32(napi_env env, int32_t v) {
    napi_value result;
    napi_create_int32(env, v, &result);
    return result;
}

static napi_value MakeInt64(napi_env env, int64_t v) {
    napi_value result;
    napi_create_int64(env, v, &result);
    return result;
}

static bool ReadString(napi_env env, napi_value val, std::string &out) {
    size_t len = 0;
    if (napi_get_value_string_utf8(env, val, nullptr, 0, &len) != napi_ok) {
        return false;
    }
    out.resize(len);
    napi_get_value_string_utf8(env, val, out.data(), len + 1, &len);
    return true;
}

static napi_value YcCoreInit(napi_env env, napi_callback_info info) {
    size_t argc = 1;
    napi_value args[1];
    napi_get_cb_info(env, info, &argc, args, nullptr, nullptr);
    std::string dir;
    if (argc > 0) {
        ReadString(env, args[0], dir);
    }
    return MakeInt32(env, yc_core_init(dir.c_str()));
}

static napi_value YcCoreShutdown(napi_env env, napi_callback_info info) {
    (void)info;
    yc_core_shutdown();
    napi_value u;
    napi_get_undefined(env, &u);
    return u;
}

static napi_value YcSessionBeginWithInput(napi_env env, napi_callback_info info) {
    size_t argc = 2;
    napi_value args[2];
    napi_get_cb_info(env, info, &argc, args, nullptr, nullptr);
    int64_t field_id = 1;
    int32_t input_type = 0;
    if (argc > 0) {
        napi_get_value_int64(env, args[0], &field_id);
    }
    if (argc > 1) {
        napi_get_value_int32(env, args[1], &input_type);
    }
    return MakeInt64(env, static_cast<int64_t>(
                              yc_session_begin_with_input(static_cast<uint64_t>(field_id),
                                                        static_cast<uint32_t>(input_type))));
}

static napi_value YcSessionValidate(napi_env env, napi_callback_info info) {
    size_t argc = 1;
    napi_value args[1];
    napi_get_cb_info(env, info, &argc, args, nullptr, nullptr);
    int64_t editor_id = 0;
    if (argc > 0) {
        napi_get_value_int64(env, args[0], &editor_id);
    }
    return MakeInt32(env, yc_session_validate(static_cast<uint64_t>(editor_id)));
}

static napi_value YcSessionStop(napi_env env, napi_callback_info info) {
    size_t argc = 2;
    napi_value args[2];
    napi_get_cb_info(env, info, &argc, args, nullptr, nullptr);
    int64_t editor_id = 0;
    int32_t reason = 0;
    if (argc > 0) {
        napi_get_value_int64(env, args[0], &editor_id);
    }
    if (argc > 1) {
        napi_get_value_int32(env, args[1], &reason);
    }
    yc_session_stop(static_cast<uint64_t>(editor_id), static_cast<uint32_t>(reason));
    napi_value u;
    napi_get_undefined(env, &u);
    return u;
}

static napi_value YcHotSubmit(napi_env env, napi_callback_info info) {
    size_t argc = 1;
    napi_value args[1];
    napi_get_cb_info(env, info, &argc, args, nullptr, nullptr);
    if (argc < 1) {
        return MakeInt32(env, YC_ERR_INTERNAL);
    }
    bool is_array = false;
    napi_is_array(env, args[0], &is_array);
    if (!is_array) {
        return MakeInt32(env, YC_ERR_INTERNAL);
    }
    uint32_t length = 0;
    napi_get_array_length(env, args[0], &length);
    if (length < sizeof(YcHotAction)) {
        return MakeInt32(env, YC_ERR_INTERNAL);
    }
    YcHotAction action{};
    for (uint32_t i = 0; i < sizeof(YcHotAction); ++i) {
        napi_value elem;
        napi_get_element(env, args[0], i, &elem);
        int32_t byte = 0;
        napi_get_value_int32(env, elem, &byte);
        reinterpret_cast<uint8_t *>(&action)[i] = static_cast<uint8_t>(byte);
    }
    return MakeInt32(env, yc_hot_submit(&action));
}

static napi_value YcHotArenaBytes(napi_env env, napi_callback_info info) {
    (void)info;
    const uint8_t *ptr = yc_hot_arena_ptr();
    const size_t size = yc_hot_arena_size();
    if (ptr == nullptr || size == 0) {
        napi_value empty;
        napi_create_array(env, &empty);
        return empty;
    }
    napi_value arr;
    napi_create_array_with_length(env, size, &arr);
    for (size_t i = 0; i < size; ++i) {
        napi_value b;
        napi_create_int32(env, ptr[i], &b);
        napi_set_element(env, arr, i, b);
    }
    return arr;
}

EXTERN_C_START
static napi_value Init(napi_env env, napi_value exports) {
    napi_property_descriptor desc[] = {
        {"ycCoreInit", nullptr, YcCoreInit, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"ycCoreShutdown", nullptr, YcCoreShutdown, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"ycSessionBeginWithInput", nullptr, YcSessionBeginWithInput, nullptr, nullptr, nullptr,
         napi_default, nullptr},
        {"ycSessionValidate", nullptr, YcSessionValidate, nullptr, nullptr, nullptr, napi_default,
         nullptr},
        {"ycSessionStop", nullptr, YcSessionStop, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"ycHotSubmit", nullptr, YcHotSubmit, nullptr, nullptr, nullptr, napi_default, nullptr},
        {"ycHotArenaBytes", nullptr, YcHotArenaBytes, nullptr, nullptr, nullptr, napi_default,
         nullptr},
    };
    napi_define_properties(env, exports, sizeof(desc) / sizeof(desc[0]), desc);
    return exports;
}
EXTERN_C_END

static napi_module ycModule = {
    .nm_version = 1,
    .nm_flags = 0,
    .nm_filename = nullptr,
    .nm_register_func = Init,
    .nm_modname = "yc_native",
    .nm_priv = nullptr,
    .reserved = {0},
};

extern "C" __attribute__((constructor)) void RegisterYcModule(void) {
    napi_module_register(&ycModule);
}
