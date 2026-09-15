cmake_minimum_required(VERSION 3.22.1)
project(yc_jni)

set(YC_HOT_INCLUDE_DIR ${CMAKE_CURRENT_SOURCE_DIR}/../jniLibs/include)
set(YC_JNI_LIBS_ABI_DIR ${YC_JNI_LIBS_DIR}/${ANDROID_ABI})
set(YC_FFI_PREBUILT ${YC_JNI_LIBS_ABI_DIR}/libyc_ffi.so)

# --- yc_jni (Rust FFI bridge) ---
add_library(yc_jni SHARED yc_jni.cpp)
target_include_directories(yc_jni PRIVATE ${YC_HOT_INCLUDE_DIR})
target_link_libraries(yc_jni log)

if (EXISTS ${YC_FFI_PREBUILT})
    message(STATUS "Linking prebuilt libyc_ffi.so for ${ANDROID_ABI}")
    add_library(yc_ffi SHARED IMPORTED)
    set_target_properties(yc_ffi PROPERTIES
        IMPORTED_LOCATION ${YC_FFI_PREBUILT}
        IMPORTED_NO_SONAME TRUE
    )
    target_link_libraries(yc_jni yc_ffi)
else ()
    message(WARNING "libyc_ffi.so not found for ${ANDROID_ABI}; using stub FFI symbols")
    target_compile_definitions(yc_jni PRIVATE YC_FFI_STUB=1)
endif ()

# --- hccr_jni (Ismantic/Handwritten NCNN) ---
set(NCNN_ROOT ${CMAKE_CURRENT_SOURCE_DIR}/../../../../third_party/ncnn-android/${ANDROID_ABI})
if (EXISTS ${NCNN_ROOT}/lib/cmake/ncnn)
    set(ncnn_DIR ${NCNN_ROOT}/lib/cmake/ncnn)
    find_package(ncnn REQUIRED)
    add_library(hccr_jni SHARED
        hccr_jni.cc
        preprocess.c
    )
    target_include_directories(hccr_jni PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
    target_link_libraries(hccr_jni PRIVATE ncnn android log)
    message(STATUS "Building hccr_jni with ncnn at ${NCNN_ROOT}")
else ()
    message(WARNING "ncnn-android missing for ${ANDROID_ABI}; hccr_jni skipped")
endif ()
