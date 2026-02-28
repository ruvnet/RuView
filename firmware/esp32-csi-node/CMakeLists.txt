# ESP32 CSI Node Firmware (ADR-018)
# Requires ESP-IDF v5.2+
cmake_minimum_required(VERSION 3.16)

set(EXTRA_COMPONENT_DIRS "")

include($ENV{IDF_PATH}/tools/cmake/project.cmake)
project(esp32-csi-node)
