cmake_minimum_required(VERSION 3.20)
project(yc-shell-linux VERSION 0.1.0 LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

if(NOT CMAKE_SYSTEM_NAME STREQUAL "Linux")
    message(FATAL_ERROR "yc-shell-linux requires Linux")
endif()

set(YC_FFI_DIR "${CMAKE_SOURCE_DIR}/libs/x64")
set(YC_FFI_SO "${YC_FFI_DIR}/libyc_ffi.so")

if(NOT EXISTS "${YC_FFI_SO}")
    message(WARNING "Missing ${YC_FFI_SO} — build yc-ffi and copy libyc_ffi.so here")
endif()

add_library(yc_ffi SHARED IMPORTED GLOBAL)
set_target_properties(yc_ffi PROPERTIES
    IMPORTED_LOCATION "${YC_FFI_SO}"
)

set(YC_UI_DESKTOP "${CMAKE_SOURCE_DIR}/../yc-ui-desktop/include")

add_library(yc_platform_adapter STATIC
    common/yc_platform_adapter.cpp
)
target_include_directories(yc_platform_adapter PUBLIC
    "${CMAKE_SOURCE_DIR}/common/include"
    "${CMAKE_SOURCE_DIR}/common"
    "${YC_UI_DESKTOP}"
)
target_link_libraries(yc_platform_adapter PUBLIC yc_ffi)

# --- IBus plugin (ibus-yc) ---
option(YC_BUILD_IBUS "Build IBus plugin" ON)
if(YC_BUILD_IBUS)
    find_package(PkgConfig QUIET)
    if(PKG_CONFIG_FOUND)
        pkg_check_modules(IBUS QUIET ibus-1.0)
    endif()

    add_library(ibus-yc MODULE
        ibus-yc/ibus_engine.c
    )
    target_include_directories(ibus-yc PRIVATE
        "${CMAKE_SOURCE_DIR}/common"
        "${CMAKE_SOURCE_DIR}/common/include"
    )
    target_link_libraries(ibus-yc PRIVATE yc_platform_adapter)
    set_target_properties(ibus-yc PROPERTIES
        OUTPUT_NAME "ibus-yc"
        PREFIX ""
        LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/lib"
    )

    if(IBUS_FOUND)
        target_include_directories(ibus-yc PRIVATE ${IBUS_INCLUDE_DIRS})
        target_link_libraries(ibus-yc PRIVATE ${IBUS_LIBRARIES})
        target_compile_definitions(ibus-yc PRIVATE YC_HAVE_IBUS=1)
    else()
        message(WARNING "ibus-1.0 not found; building ibus-yc M0 stub without IBus headers")
        target_compile_definitions(ibus-yc PRIVATE YC_IBUS_STUB=1)
    endif()
endif()

# --- Fcitx5 addon (fcitx5-yc) ---
option(YC_BUILD_FCITX5 "Build Fcitx5 addon" ON)
if(YC_BUILD_FCITX5)
    find_package(PkgConfig QUIET)
    if(PKG_CONFIG_FOUND)
        pkg_check_modules(FCITX5 QUIET fcitx5)
    endif()

    add_library(fcitx5-yc MODULE
        fcitx5-yc/fcitx5_addon.cpp
    )
    target_include_directories(fcitx5-yc PRIVATE
        "${CMAKE_SOURCE_DIR}/common"
        "${CMAKE_SOURCE_DIR}/common/include"
    )
    target_link_libraries(fcitx5-yc PRIVATE yc_platform_adapter)
    set_target_properties(fcitx5-yc PROPERTIES
        OUTPUT_NAME "fcitx5-yc"
        PREFIX ""
        LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/lib"
    )

    if(FCITX5_FOUND)
        target_include_directories(fcitx5-yc PRIVATE ${FCITX5_INCLUDE_DIRS})
        target_link_libraries(fcitx5-yc PRIVATE ${FCITX5_LIBRARIES})
        target_compile_definitions(fcitx5-yc PRIVATE YC_HAVE_FCITX5=1)
    else()
        message(WARNING "fcitx5 not found; building fcitx5-yc M0 stub without Fcitx5 headers")
        target_compile_definitions(fcitx5-yc PRIVATE YC_FCITX5_STUB=1)
    endif()
endif()
