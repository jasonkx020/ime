#include <jni.h>

#include <cstdint>
#include <cstring>
#include <vector>

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

int32_t yc_hw_push_stroke(uint64_t editor_id, const YcStrokePoint *points, uint32_t point_count,
                          uint64_t session_stroke_id, uint32_t canvas_width, uint32_t canvas_height,
                          uint32_t writing_mode) {
    (void)editor_id;
    (void)points;
    (void)point_count;
    (void)session_stroke_id;
    (void)canvas_width;
    (void)canvas_height;
    (void)writing_mode;
    return YC_OK;
}

int32_t yc_hw_apply_result(uint64_t editor_id, uint32_t count, const uint8_t *texts,
                           uint32_t texts_bytes, const float *scores, uint32_t flags) {
    (void)editor_id;
    (void)count;
    (void)texts;
    (void)texts_bytes;
    (void)scores;
    (void)flags;
    return YC_OK;
}

int32_t yc_core_install_langpack(const char *pack_path) {
    (void)pack_path;
    return YC_OK;
}

int32_t yc_personalization_apply(const char *pairs_json, const char *deltas_json,
                                 const char *boosts_json) {
    (void)pairs_json;
    (void)deltas_json;
    (void)boosts_json;
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

extern "C" JNIEXPORT jint JNICALL
Java_com_yc_input_native_YcNative_ycCoreInstallLangpack(JNIEnv *env, jclass, jstring pack_path) {
    const char *path = env->GetStringUTFChars(pack_path, nullptr);
    const jint rc = yc_core_install_langpack(path);
    env->ReleaseStringUTFChars(pack_path, path);
    return rc;
}

/**
 * Push one stroke. xyPressure: [x,y,pressure] * N (normalized 0..1).
 * timesMs: timestamp per point (length N).
 */
extern "C" JNIEXPORT jint JNICALL
Java_com_yc_input_native_YcNative_ycHwPushStroke(
    JNIEnv *env, jclass, jlong editor_id, jfloatArray xy_pressure, jlongArray times_ms,
    jlong session_stroke_id, jint canvas_w, jint canvas_h, jint writing_mode) {
    if (xy_pressure == nullptr || times_ms == nullptr) {
        return YC_ERR_INTERNAL;
    }
    const jsize n_times = env->GetArrayLength(times_ms);
    const jsize n_xy = env->GetArrayLength(xy_pressure);
    if (n_times <= 0 || n_xy != n_times * 3 || n_times > YC_MAX_HW_POINTS) {
        return YC_ERR_INTERNAL;
    }
    jfloat *xy = env->GetFloatArrayElements(xy_pressure, nullptr);
    jlong *ts = env->GetLongArrayElements(times_ms, nullptr);
    if (xy == nullptr || ts == nullptr) {
        if (xy) env->ReleaseFloatArrayElements(xy_pressure, xy, JNI_ABORT);
        if (ts) env->ReleaseLongArrayElements(times_ms, ts, JNI_ABORT);
        return YC_ERR_INTERNAL;
    }
    YcStrokePoint pts[YC_MAX_HW_POINTS];
    for (jsize i = 0; i < n_times; ++i) {
        pts[i].x = xy[i * 3];
        pts[i].y = xy[i * 3 + 1];
        pts[i].pressure = xy[i * 3 + 2];
        pts[i].t = static_cast<uint64_t>(ts[i]);
    }
    const jint rc = yc_hw_push_stroke(
        static_cast<uint64_t>(editor_id), pts, static_cast<uint32_t>(n_times),
        static_cast<uint64_t>(session_stroke_id), static_cast<uint32_t>(canvas_w),
        static_cast<uint32_t>(canvas_h), static_cast<uint32_t>(writing_mode));
    env->ReleaseFloatArrayElements(xy_pressure, xy, JNI_ABORT);
    env->ReleaseLongArrayElements(times_ms, ts, JNI_ABORT);
    return rc;
}

/**
 * Apply shell handwriting OCR results.
 * texts: String[] length N; scores: float[N]; flags bit0 = needs_cloud_confirm.
 */
extern "C" JNIEXPORT jint JNICALL
Java_com_yc_input_native_YcNative_ycHwApplyResult(
    JNIEnv *env, jclass, jlong editor_id, jobjectArray texts, jfloatArray scores, jint flags) {
    if (texts == nullptr || scores == nullptr) {
        return YC_ERR_INTERNAL;
    }
    const jsize n = env->GetArrayLength(texts);
    const jsize n_scores = env->GetArrayLength(scores);
    if (n <= 0 || n > 200 || n != n_scores) {
        return YC_ERR_INTERNAL;
    }
    jfloat *score_elems = env->GetFloatArrayElements(scores, nullptr);
    if (score_elems == nullptr) {
        return YC_ERR_INTERNAL;
    }
    std::vector<uint8_t> blob;
    blob.reserve(static_cast<size_t>(n) * 8);
    for (jsize i = 0; i < n; ++i) {
        auto jstr = static_cast<jstring>(env->GetObjectArrayElement(texts, i));
        if (jstr == nullptr) {
            blob.push_back(0);
            continue;
        }
        const char *utf = env->GetStringUTFChars(jstr, nullptr);
        if (utf) {
            const size_t len = std::strlen(utf);
            blob.insert(blob.end(), utf, utf + len);
            env->ReleaseStringUTFChars(jstr, utf);
        }
        blob.push_back(0);
        env->DeleteLocalRef(jstr);
    }
    const jint rc = yc_hw_apply_result(
        static_cast<uint64_t>(editor_id), static_cast<uint32_t>(n), blob.data(),
        static_cast<uint32_t>(blob.size()), score_elems, static_cast<uint32_t>(flags));
    env->ReleaseFloatArrayElements(scores, score_elems, JNI_ABORT);
    return rc;
}

extern "C" JNIEXPORT jint JNICALL
Java_com_yc_input_native_YcNative_ycPersonalizationApply(JNIEnv *env, jclass, jstring pairs_json,
                                                         jstring deltas_json, jstring boosts_json) {
    const char *pairs = pairs_json ? env->GetStringUTFChars(pairs_json, nullptr) : nullptr;
    const char *deltas = deltas_json ? env->GetStringUTFChars(deltas_json, nullptr) : nullptr;
    const char *boosts = boosts_json ? env->GetStringUTFChars(boosts_json, nullptr) : nullptr;
    const jint rc = yc_personalization_apply(pairs, deltas, boosts);
    if (pairs_json && pairs) {
        env->ReleaseStringUTFChars(pairs_json, pairs);
    }
    if (deltas_json && deltas) {
        env->ReleaseStringUTFChars(deltas_json, deltas);
    }
    if (boosts_json && boosts) {
        env->ReleaseStringUTFChars(boosts_json, boosts);
    }
    return rc;
}
