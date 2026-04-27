# ESP32 CSI Node Firmware (ADR-018)
# Requires ESP-IDF v5.2+
cmake_minimum_required(VERSION 3.16)

set(EXTRA_COMPONENT_DIRS "")

# Read firmware version from version.txt so esp_app_get_description()->version
# matches the release tag.  Fixes issue #354 (version mismatch after flashing).
file(STRINGS "${CMAKE_CURRENT_LIST_DIR}/version.txt" PROJECT_VER LIMIT_COUNT 1)
string(STRIP "${PROJECT_VER}" PROJECT_VER)

include($ENV{IDF_PATH}/tools/cmake/project.cmake)
project(esp32-csi-node VERSION ${PROJECT_VER})
