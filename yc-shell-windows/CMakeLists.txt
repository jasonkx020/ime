cmake_minimum_required(VERSION 3.20)
project(yc-shell-windows VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if(NOT WIN32)
    message(FATAL_ERROR "yc-shell-windows requires Windows")
endif()

set(YC_FFI_DIR "${CMAKE_SOURCE_DIR}/libs/x64")
set(YC_FFI_DLL "${YC_FFI_DIR}/yc_ffi.dll")
set(YC_FFI_IMPLIB "${YC_FFI_DIR}/yc_ffi.dll.lib")

if(NOT EXISTS "${YC_FFI_DLL}")
    message(WARNING "Missing ${YC_FFI_DLL} — run yc-core/scripts/build-desktop.ps1 first")
endif()

add_library(yc_ffi SHARED IMPORTED GLOBAL)
set_target_properties(yc_ffi PROPERTIES
    IMPORTED_LOCATION "${YC_FFI_DLL}"
    IMPORTED_IMPLIB "${YC_FFI_IMPLIB}"
)

set(YC_UI_DESKTOP "${CMAKE_SOURCE_DIR}/../yc-ui-desktop/include")

add_library(yc_tip SHARED
    yc_tip/yc_platform_adapter.cpp
    yc_tip/yc_tsfp.cpp
)
target_include_directories(yc_tip PRIVATE
    "${CMAKE_SOURCE_DIR}/yc_tip/include"
    "${YC_UI_DESKTOP}"
)
target_link_libraries(yc_tip PRIVATE yc_ffi)
set_target_properties(yc_tip PROPERTIES
    OUTPUT_NAME "yc_tip"
    RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/bin"
    LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/bin"
)

add_custom_command(TARGET yc_tip POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different
        "${YC_FFI_DLL}"
        "$<TARGET_FILE_DIR:yc_tip>"
    COMMENT "Copy yc_ffi.dll next to yc_tip.dll"
)
