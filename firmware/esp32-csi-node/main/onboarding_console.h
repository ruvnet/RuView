#ifndef RUVIEW_ONBOARDING_CONSOLE_H
#define RUVIEW_ONBOARDING_CONSOLE_H

#include "esp_err.h"
#include "esp_idf_version.h"

#if defined(CONFIG_ESP_CONSOLE_USB_SERIAL_JTAG) && ESP_IDF_VERSION_MAJOR >= 6
#include "driver/usb_serial_jtag.h"
#include "driver/usb_serial_jtag_vfs.h"
#endif

static inline esp_err_t ruview_onboarding_console_prepare(void)
{
#if defined(CONFIG_ESP_CONSOLE_USB_SERIAL_JTAG) && ESP_IDF_VERSION_MAJOR >= 6
    /* IDF 6 VFS nonblocking reads consult the driver's RX ring buffer even
     * when the default polling console has not installed that driver. Boot
     * output works in that state, but every onboarding read stays empty. */
    if (!usb_serial_jtag_is_driver_installed()) {
        usb_serial_jtag_driver_config_t config = USB_SERIAL_JTAG_DRIVER_CONFIG_DEFAULT();
        const esp_err_t result = usb_serial_jtag_driver_install(&config);
        if (result != ESP_OK) return result;
    }
    usb_serial_jtag_vfs_use_driver();
#endif
    return ESP_OK;
}

#endif
