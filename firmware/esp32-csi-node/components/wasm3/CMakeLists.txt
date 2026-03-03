# WASM3 — WebAssembly interpreter for ESP-IDF
#
# ADR-040: Tier 3 WASM programmable sensing layer.
# WASM3 is an MIT-licensed, lightweight interpreter (~100 KB flash, ~10 KB RAM)
# optimized for embedded targets including Xtensa ESP32-S3.
#
# Pre-download WASM3 source before building:
#   cd firmware/esp32-csi-node/components/wasm3
#   git clone --depth 1 https://github.com/wasm3/wasm3.git wasm3-src
#
# Or run: scripts/fetch-wasm3.sh

cmake_minimum_required(VERSION 3.16)

set(WASM3_DIR "${CMAKE_CURRENT_SOURCE_DIR}/wasm3-src")

if(NOT EXISTS "${WASM3_DIR}/source/wasm3.h")
    message(STATUS "WASM3 source not found at ${WASM3_DIR}")
    message(STATUS "Attempting to download WASM3...")

    # Try downloading inside build environment.
    set(WASM3_URL "https://github.com/nicholasgasior/wasm3/archive/refs/heads/main.tar.gz")
    set(WASM3_ARCHIVE "${CMAKE_CURRENT_BINARY_DIR}/wasm3.tar.gz")

    file(DOWNLOAD "${WASM3_URL}" "${WASM3_ARCHIVE}"
         STATUS DOWNLOAD_STATUS TIMEOUT 30)
    list(GET DOWNLOAD_STATUS 0 DL_CODE)

    if(DL_CODE EQUAL 0)
        execute_process(
            COMMAND ${CMAKE_COMMAND} -E tar xzf "${WASM3_ARCHIVE}"
            WORKING_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}")
        file(GLOB WASM3_EXTRACTED "${CMAKE_CURRENT_BINARY_DIR}/wasm3-*")
        if(WASM3_EXTRACTED)
            list(GET WASM3_EXTRACTED 0 WASM3_EXTRACTED_DIR)
            file(RENAME "${WASM3_EXTRACTED_DIR}" "${WASM3_DIR}")
        endif()
        file(REMOVE "${WASM3_ARCHIVE}")
    endif()

    if(NOT EXISTS "${WASM3_DIR}/source/wasm3.h")
        message(WARNING "WASM3 source not available. Building WITHOUT WASM Tier 3 support.\n"
                        "To enable: git clone --depth 1 https://github.com/wasm3/wasm3.git "
                        "${WASM3_DIR}")
        # Register empty component so ESP-IDF doesn't error.
        idf_component_register()
        return()
    endif()
endif()

# Collect all WASM3 source files.
file(GLOB WASM3_SOURCES "${WASM3_DIR}/source/*.c")

idf_component_register(
    SRCS ${WASM3_SOURCES}
    INCLUDE_DIRS "${WASM3_DIR}/source"
)

# WASM3 configuration for ESP32-S3 Xtensa target.
target_compile_definitions(${COMPONENT_LIB} PUBLIC
    d_m3HasFloat=1                   # Enable float support (needed for DSP)
    d_m3Use32BitSlots=1              # 32-bit value slots (saves RAM on ESP32)
    d_m3MaxFunctionStackHeight=512   # Raised for Rust WASM modules (was 128)
    d_m3CodePageAlignSize=4096       # Page alignment for Xtensa
    d_m3LogOutput=0                  # Disable WASM3 stdout logging (use ESP_LOG)
    d_m3FixedHeap=0                  # Use dynamic allocation (PSRAM-friendly)
    WASM3_AVAILABLE=1                # Flag for conditional compilation
)

# Suppress warnings from third-party code.
target_compile_options(${COMPONENT_LIB} PRIVATE
    -Wno-unused-function
    -Wno-unused-variable
    -Wno-maybe-uninitialized
    -Wno-sign-compare
)
