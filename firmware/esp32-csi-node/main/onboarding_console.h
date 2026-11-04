#ifndef RUVIEW_ONBOARDING_CONSOLE_H
#define RUVIEW_ONBOARDING_CONSOLE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include "esp_err.h"
#include "esp_idf_version.h"

#if defined(CONFIG_RUVIEW_ONBOARDING_NATIVE_USB) || (defined(CONFIG_ESP_CONSOLE_USB_SERIAL_JTAG) && ESP_IDF_VERSION_MAJOR >= 6)
#include "freertos/FreeRTOS.h"
#include "driver/usb_serial_jtag.h"
#include "driver/usb_serial_jtag_vfs.h"
#define RUVIEW_ONBOARDING_NEEDS_USB_DRIVER 1
#endif

/* This is an additional physical endpoint, never a second reader of primary
 * USB stdin. Profiles opt in only with a UART primary, preserving that port. */
#if defined(CONFIG_RUVIEW_ONBOARDING_NATIVE_USB) && defined(CONFIG_ESP_CONSOLE_UART)
#define RUVIEW_ONBOARDING_HAS_NATIVE_USB 1
#else
#define RUVIEW_ONBOARDING_HAS_NATIVE_USB 0
#endif

static inline esp_err_t ruview_onboarding_console_prepare(void)
{
#ifdef RUVIEW_ONBOARDING_NEEDS_USB_DRIVER
    /* IDF 6 primary VFS input needs its RX ring installed; an additional USB
     * endpoint also uses that driver directly. Do not reinstall an owner. */
    if (!usb_serial_jtag_is_driver_installed()) {
        usb_serial_jtag_driver_config_t config = USB_SERIAL_JTAG_DRIVER_CONFIG_DEFAULT();
#if RUVIEW_ONBOARDING_HAS_NATIVE_USB
        config.rx_buffer_size = 512;
        config.tx_buffer_size = 1024;
#endif
        const esp_err_t result = usb_serial_jtag_driver_install(&config);
        if (result != ESP_OK) return result;
    }
    /* Existing mirrored console output must share the driver's TX queue with
     * replies, rather than competing with its ISR via polling writes. */
    usb_serial_jtag_vfs_use_driver();
#endif
    return ESP_OK;
}

static inline int ruview_onboarding_usb_read(uint8_t *bytes, size_t capacity)
{
#if RUVIEW_ONBOARDING_HAS_NATIVE_USB
    if (!usb_serial_jtag_is_driver_installed()) return 0;
    return usb_serial_jtag_read_bytes(bytes, (uint32_t)capacity, 0);
#else
    (void)bytes;
    (void)capacity;
    return 0;
#endif
}

static inline bool ruview_onboarding_usb_write(const char *bytes, size_t length)
{
#if RUVIEW_ONBOARDING_HAS_NATIVE_USB
    /* One bounded driver operation. A full TX queue fails closed; it cannot
     * hold the UART endpoint or CSI processing indefinitely. */
    if (!usb_serial_jtag_is_driver_installed()) return false;
    return usb_serial_jtag_write_bytes(bytes, length, pdMS_TO_TICKS(20)) == (int)length;
#else
    (void)bytes;
    (void)length;
    return false;
#endif
}

#endif
