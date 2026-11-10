# Experimental adapter depends on these exact vendor inputs. Never silently
# apply binary-derived register semantics to a different SDK/library/header.
function(ruview_c5_require_sdk_input relative expected)
    file(SHA256 "${IDF_PATH}/${relative}" actual)
    if(NOT actual STREQUAL expected)
        message(FATAL_ERROR "C5 LLTF8 private readback SDK input mismatch: ${relative}")
    endif()
endfunction()
ruview_c5_require_sdk_input("components/esp_wifi/include/esp_wifi_he_types.h" "602abd515ad617d06d8fe01e94fdf755890e72194a56a03821fa49770544162d")
ruview_c5_require_sdk_input("components/esp_wifi/lib/esp32c5/libnet80211.a" "0bcff75e0e875aaa5bb0a3b3862fccc234b0620925f2fec6e49f5f31d189a205")
ruview_c5_require_sdk_input("components/esp_wifi/lib/esp32c5/libpp.a" "df7d4d05a7e1664ecd9d07c1274220cfbe0d5bf93c30c87e3556815c928f3aa9")
ruview_c5_require_sdk_input("components/esp_phy/lib/esp32c5/libphy.a" "dbf33c418c8d408d4005c849d12a1432deea82e2e5e57de3c8ddf914d104fffb")
