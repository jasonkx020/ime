#include <jni.h>

#include <cstdint>
#include <cstring>

#include "yc_hot.h"

#if defined(YC_FFI_STUB)
extern "C" {

static uint8_t g_stub_arena[8192]{};

int32_t yc_core_init(const char *data_dir) {
    (void)data_dir;
    return YC_OK;
}

void yc_core_shutdown(void) {}

uint64_t yc_session_begin(uint64_t field_id) { return field_id == 0 ? 1 : field_id; }

uint64_t yc_session_begin_with_input(uint64_t field_id, uint32_t input_type) {
    (void)input_type;
    return yc_session_begin(field_id);
}

int32_t yc_session_validate(uint64_t editor_id) { return editor_id != 0 ? 1 : 0; }

void yc_session_stop(uint64_t editor_id, uint32_t reason) {
    (void)editor_id;
    (void)reason;
}

int32_t yc_hot_submit(const YcHotAction *action) {
    (void)action;
    return YC_OK;
}

const uint8_t *yc_hot_arena_ptr(void) { return g_stub_arena; }

size_t yc_hot_arena_size(void) { return sizeof(g_stub_arena); }

int32_t yc_hot_latest_seq(uint64_t editor_id, uint64_t *out_seq) {
    (void)editor_id;
    if (out_seq) {
        *out_seq = 0;
    }
    return YC_OK;
}

} // extern "C"
#endif

extern "C" JNIEXPORT jint JNICALL
Java_com_yc_input_native_YcNative_ycCoreInit(JNIEnv *env, jclass, jstring data_dir) {
    const char *dir = env->GetStringUTFChars(data_dir, nullptr);
    const jint rc = yc_core_init(dir);
    env->ReleaseStringUTFChars(data_dir, dir);
    return rc;
}

extern "C" JNIEXPORT void JNICALL
Java_com_yc_input_native_YcNative_ycCoreShutdown(JNIEnv *, jclass) {
    yc_core_shutdown();
}

extern "C" JNIEXPORT jlong JNICALL
Java_com_yc_input_native_YcNative_ycSessionBegin(JNIEnv *, jclass, jlong field_id) {
    return static_cast<jlong>(yc_session_begin(static_cast<uint64_t>(field_id)));
}

extern "C" JNIEXPORT jlong JNICALL
Java_com_yc_input_native_YcNative_ycSessionBeginWithInput(JNIEnv *, jclass, jlong field_id,
                                                          jint input_type) {
    return static_cast<jlong>(
        yc_session_begin_with_input(static_cast<uint64_t>(field_id), static_cast<uint32_t>(input_type)));
}

extern "C" JNIEXPORT jint JNICALL
Java_com_yc_input_native_YcNative_ycSessionValidate(JNIEnv *, jclass, jlong editor_id) {
    return yc_session_validate(static_cast<uint64_t>(editor_id));
}

extern "C" JNIEXPORT void JNICALL
Java_com_yc_input_native_YcNative_ycSessionStop(JNIEnv *, jclass, jlong editor_id, jint reason) {
    yc_session_stop(static_cast<uint64_t>(editor_id), static_cast<uint32_t>(reason));
}

extern "C" JNIEXPORT jint JNICALL
Java_com_yc_input_native_YcNative_ycHotSubmit(JNIEnv *env, jclass, jbyteArray action_bytes) {
    if (action_bytes == nullptr) {
        return YC_ERR_INTERNAL;
    }
    const jsize len = env->GetArrayLength(action_bytes);
    if (len < static_cast<jsize>(sizeof(YcHotAction))) {
        return YC_ERR_INTERNAL;
    }
    YcHotAction action{};
    env->GetByteArrayRegion(action_bytes, 0, sizeof(YcHotAction),
                            reinterpret_cast<jbyte *>(&action));
    return yc_hot_submit(&action);
}

extern "C" JNIEXPORT jlong JNICALL
Java_com_yc_input_native_YcNative_ycHotArenaPtr(JNIEnv *, jclass) {
    return reinterpret_cast<jlong>(yc_hot_arena_ptr());
}

extern "C" JNIEXPORT jint JNICALL
Java_com_yc_input_native_YcNative_ycHotArenaSize(JNIEnv *, jclass) {
    return static_cast<jint>(yc_hot_arena_size());
}

extern "C" JNIEXPORT jlong JNICALL
Java_com_yc_input_native_YcNative_ycHotLatestSeq(JNIEnv *, jclass, jlong editor_id) {
    uint64_t seq = 0;
    if (yc_hot_latest_seq(static_cast<uint64_t>(editor_id), &seq) != YC_OK) {
        return 0;
    }
    return static_cast<jlong>(seq);
}

extern "C" JNIEXPORT void JNICALL
Java_com_yc_input_native_YcNative_nativeReadBytes(JNIEnv *env, jclass, jlong ptr, jbyteArray dest,
                                                  jint size) {
    if (ptr == 0 || dest == nullptr || size <= 0) {
        return;
    }
    env->SetByteArrayRegion(dest, 0, size, reinterpret_cast<const jbyte *>(ptr));
}

extern "C" JNIEXPORT jint JNICALL
Java_com_yc_input_native_YcNative_ycColdSubmit(JNIEnv *env, jclass, jlong editor_id, jint kind,
                                               jbyteArray payload) {
    if (payload == nullptr) {
        return yc_cold_submit(static_cast<uint64_t>(editor_id), static_cast<uint32_t>(kind),
                              nullptr, 0);
    }
    const jsize len = env->GetArrayLength(payload);
    jbyte *bytes = env->GetByteArrayElements(payload, nullptr);
    const jint rc = yc_cold_submit(static_cast<uint64_t>(editor_id), static_cast<uint32_t>(kind),
                                   reinterpret_cast<const uint8_t *>(bytes),
                                   static_cast<size_t>(len));
    env->ReleaseByteArrayElements(payload, bytes, JNI_ABORT);
    return rc;
}

extern "C" JNIEXPORT jint JNICALL
Java_com_yc_input_native_YcNative_ycCoreSyncLangPacks(JNIEnv *, jclass) {
    return yc_core_sync_lang_packs();
}
