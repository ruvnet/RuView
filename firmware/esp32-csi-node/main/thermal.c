/* On-chip thermal monitoring and radio throttling.
 *
 * The ESP32-C6 carries a temperature sensor (SOC_TEMP_SENSOR_SUPPORTED) that
 * nothing in this firmware has ever read. That was tolerable on a bare board
 * on a bench; it is not once nine nodes are sealed in printed enclosures and
 * left on a wall for months with the radio transmitting continuously.
 *
 * What we are protecting is the enclosure, not the silicon. The part is rated
 * well past anything it will see here, but PLA goes soft at its glass
 * transition around 55-60 C -- and that is sagging under its own weight over
 * weeks, not a dramatic failure, so it would never announce itself.
 *
 * The sensor reads DIE temperature, which runs hotter than the plastic around
 * it. Thresholds are therefore set against the die with that offset in mind:
 * a die at ~72 C in a small vented box is roughly a 50-55 C enclosure, which
 * is where PLA starts to matter. They are deliberately not set at 55 C -- that
 * would throttle a perfectly healthy node.
 *
 * Throttling reduces WiFi transmit power, which is the dominant heat source
 * and degrades gracefully: links weaken but keep working, where cutting the
 * capture rate would blind the node entirely.
 *
 * IMPORTANT for anyone reading link diagnostics: a throttled node's RSSI drops
 * at both ends, which looks exactly like an obstruction. `thermal_state()` and
 * `thermal_tx_dbm()` exist so that can be distinguished from a cage or a wall
 * rather than misdiagnosed as one.
 */

#include "sdkconfig.h"
#include "thermal.h"

#if defined(CONFIG_THERMAL_MONITOR) && defined(CONFIG_IDF_TARGET_ESP32C6)

#include "driver/temperature_sensor.h"
#include "esp_wifi.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "esp_system.h"
#include "esp_heap_caps.h"

static const char *TAG = "thermal";

static temperature_sensor_handle_t s_sensor;
static bool     s_ready;
static float    s_last_c    = -273.0f;
static float    s_peak_c    = -273.0f;
static thermal_state_t s_state = THERMAL_OK;

/* esp_wifi_set_max_tx_power() takes 0.25 dBm units. 80 = 20 dBm, the default
 * on this part; each step down is 3 dB, which halves radiated power. */
#define TXP_FULL   80   /* 20 dBm */
#define TXP_STEP1  68   /* 17 dBm */
#define TXP_STEP2  56   /* 14 dBm */
#define TXP_MIN    44   /* 11 dBm */

static int8_t s_txp = TXP_FULL;

#ifdef CONFIG_THERMAL_THROTTLE
/* Only referenced from the throttling block in thermal_tick(). Guarded so a
 * monitor-only build (the default, and what is on the wall) does not warn
 * about an unused function. */
static void apply_tx_power(int8_t q)
{
    if (q == s_txp) {
        return;
    }
    esp_err_t r = esp_wifi_set_max_tx_power(q);
    if (r == ESP_OK) {
        ESP_LOGW(TAG, "tx power %d.%02d dBm -> %d.%02d dBm (thermal)",
                 s_txp / 4, (s_txp % 4) * 25, q / 4, (q % 4) * 25);
        s_txp = q;
    } else {
        ESP_LOGW(TAG, "esp_wifi_set_max_tx_power(%d) failed: %s",
                 q, esp_err_to_name(r));
    }
}
#endif /* CONFIG_THERMAL_THROTTLE */

esp_err_t thermal_init(void)
{
    /* The requested span must fit ENTIRELY inside one of the sensor's
     * predefined measurement ranges -- the driver walks a table and takes the
     * first that contains it, so a span crossing two boundaries is rejected
     * with ESP_ERR_INVALID_ARG rather than clamped. (-10,110) looks reasonable
     * and is invalid for exactly that reason; it failed on real hardware.
     *
     * (20,100) is the right first choice here: the warn/throttle/critical
     * thresholds all sit comfortably inside it, and indoor ambient is
     * essentially never below 20 C. The wider fallbacks exist so an unheated
     * room, or a different chip revision with a different table, degrades to a
     * usable range instead of leaving the node unmonitored. */
    static const struct { int lo, hi; } spans[] = {
        {20, 100}, {-10, 80}, {-30, 50}, {50, 125},
    };
    esp_err_t r = ESP_ERR_INVALID_ARG;
    for (size_t i = 0; i < sizeof(spans) / sizeof(spans[0]); i++) {
        temperature_sensor_config_t cfg =
            TEMPERATURE_SENSOR_CONFIG_DEFAULT(spans[i].lo, spans[i].hi);
        r = temperature_sensor_install(&cfg, &s_sensor);
        if (r == ESP_OK) {
            ESP_LOGI(TAG, "sensor range %d..%d C", spans[i].lo, spans[i].hi);
            break;
        }
    }
    if (r != ESP_OK) {
        ESP_LOGW(TAG, "sensor install failed on every range: %s "
                      "(continuing unmonitored)", esp_err_to_name(r));
        return r;
    }
    r = temperature_sensor_enable(s_sensor);
    if (r != ESP_OK) {
        ESP_LOGW(TAG, "sensor enable failed: %s", esp_err_to_name(r));
        return r;
    }
    s_ready = true;
    ESP_LOGI(TAG, "monitoring on: warn %d C, throttle %d C, critical %d C "
                  "(die temperature; protecting the enclosure, not the chip)",
             CONFIG_THERMAL_WARN_C, CONFIG_THERMAL_THROTTLE_C,
             CONFIG_THERMAL_CRITICAL_C);
    return ESP_OK;
}

void thermal_tick(void)
{
    if (!s_ready) {
        return;
    }
    float c;
    if (temperature_sensor_get_celsius(s_sensor, &c) != ESP_OK) {
        return;
    }
    s_last_c = c;
    if (c > s_peak_c) {
        s_peak_c = c;
    }

    /* Hysteresis on the way down: recovering at the same threshold that
     * tripped would oscillate, and each oscillation is a visible RSSI step on
     * every link this node has. The decision itself lives in thermal.h as a
     * pure function so a host test can exercise the same code this runs. */
    const thermal_state_t next = thermal_next_state(s_state, c,
                                                    CONFIG_THERMAL_WARN_C,
                                                    CONFIG_THERMAL_THROTTLE_C,
                                                    CONFIG_THERMAL_CRITICAL_C,
                                                    CONFIG_THERMAL_HYSTERESIS_C);

    if (next != s_state) {
        ESP_LOGW(TAG, "%.1f C: %s -> %s",
                 c, thermal_state_name(s_state), thermal_state_name(next));
        s_state = next;
    }

#ifdef CONFIG_THERMAL_THROTTLE
    /* Acting on the reading is separate from taking it. Measuring is inert;
     * changing transmit power mid-experiment moves every RSSI this node has,
     * which is not something to introduce during a fleet bring-up when any
     * surprise needs a single candidate cause. Turn this on once the fleet is
     * stable AND the logs show a node actually gets hot -- if none ever does,
     * it never needs turning on at all. */
    switch (s_state) {
    case THERMAL_CRITICAL:  apply_tx_power(TXP_MIN);   break;
    case THERMAL_THROTTLED: apply_tx_power(TXP_STEP2); break;
    case THERMAL_WARN:      apply_tx_power(TXP_STEP1); break;
    case THERMAL_OK:        apply_tx_power(TXP_FULL);  break;
    }
#endif

    static int64_t s_last_log_us;
    int64_t now = esp_timer_get_time();
    if (now - s_last_log_us > 60LL * 1000 * 1000) {
        s_last_log_us = now;
        /* Heap rides along because it is the other thing that fails slowly
         * and silently over weeks. A node that reboots mysteriously after ten
         * days looks identical to one that overheated, unless the minimum
         * free heap has been visibly walking downwards the whole time. */
        ESP_LOGI(TAG, "die %.1f C (peak %.1f C) state=%s tx=%d.%02d dBm "
                      "heap %u min %u",
                 s_last_c, s_peak_c, thermal_state_name(s_state),
                 s_txp / 4, (s_txp % 4) * 25,
                 (unsigned)esp_get_free_heap_size(),
                 (unsigned)esp_get_minimum_free_heap_size());
    }
}

float thermal_celsius(void)      { return s_last_c; }
float thermal_peak_celsius(void) { return s_peak_c; }
thermal_state_t thermal_state(void) { return s_state; }
int8_t thermal_tx_dbm(void)      { return (int8_t)(s_txp / 4); }

const char *thermal_state_name(thermal_state_t s)
{
    switch (s) {
    case THERMAL_OK:        return "ok";
    case THERMAL_WARN:      return "warn";
    case THERMAL_THROTTLED: return "throttled";
    case THERMAL_CRITICAL:  return "critical";
    }
    return "?";
}

#endif /* CONFIG_THERMAL_MONITOR && ESP32C6 */
