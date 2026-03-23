/**
 * @file led_indicator.h
 * @brief Configurable RGB LED Status Indicator for ESP32 CSI Node
 */

#ifndef LED_INDICATOR_H
#define LED_INDICATOR_H

#ifdef __cplusplus
extern "C" {
#endif

/** State of the system to indicate via the LED */
typedef enum {
    LED_STATE_BOOTING = 0,
    LED_STATE_WIFI_CONNECTING,
    LED_STATE_CONNECTED,
    LED_STATE_WIFI_ERROR,
    LED_STATE_MOCK_MODE,
    LED_STATE_MMWAVE_ERROR,
    LED_STATE_SWARM_ERROR,
    LED_STATE_SWARM_ACTIVE,
} led_indicator_state_t;

/**
 * Initializes the LED indicator system if enabled via NVS and Kconfig.
 * Starts the background FreeRTOS task to drive the NeoPixel animations.
 */
void led_indicator_init(void);

/**
 * Updates the current system state, changing the LED animation.
 * @param state The new system state to indicate.
 */
void led_indicator_set_state(led_indicator_state_t state);

#ifdef __cplusplus
}
#endif

#endif /* LED_INDICATOR_H */
