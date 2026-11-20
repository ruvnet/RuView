/* Persistent on-node log -- survives a power cycle, readable without a cable.
 *
 * The problem this exists for (Joe, 2026-09-07): "a hung node ... doesnt do us
 * any good for troubleshooting remotly because the unplug and mount results in
 * a clearing of the condtion." Today the post-mortem for any remote fault is
 * nothing: main.c decodes the reset reason at boot and logs it to the CONSOLE,
 * which is exactly what a power cycle destroys.
 *
 * The flash was already partitioned for this and the features were off --
 * partitions_16mb.csv reserves a 64K coredump partition and an 8000K storage
 * FAT volume, neither of which had ever been mounted or written.
 *
 * # Retention, not wear, is the binding constraint
 *
 * 128 B records every 300 s fills 8 MB in ~222 days. Endurance at that rate is
 * ~61,000 years, so the ring wraps 36 million times before the flash is at
 * risk. The write rate is nonetheless bounded explicitly below, because flash
 * wear is the one way this module can do harm.
 *
 * # Why uptime and boot_id rather than a timestamp
 *
 * These boards have no RTC. Nothing can be timestamped absolutely, so ordering
 * comes from boot_id (incremented once per boot, persisted in NVS) plus
 * uptime_s within that boot. That ordering survives a power cycle; a wall clock
 * would not.
 *
 * # Counters are stored RAW
 *
 * Every counter here is cumulative and is written as-is. Differencing happens
 * at read time. Storing rates instead would bake in the sampling interval and
 * make every record useless the moment the cadence changed.
 */
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

/** Every record is exactly this size, so the ring is index-addressable and a
 *  reader never has to parse forward to find a boundary. */
#define NODE_LOG_RECORD_SIZE 128

/** Record types. A reader dispatches on this byte alone. */
#define NODE_LOG_TYPE_PERIODIC 1
#define NODE_LOG_TYPE_BOOT     2
#define NODE_LOG_TYPE_EVENT    3

/** Event subtypes. Periodic samples give the trend; these give the moment. */
#define NODE_LOG_EV_WIFI_DISCONNECT 1
#define NODE_LOG_EV_WIFI_RECONNECT  2
#define NODE_LOG_EV_THERMAL_STATE   3
#define NODE_LOG_EV_SEND_FAIL_RATE  4
#define NODE_LOG_EV_WATCHDOG        5

/** Mount the FAT volume and open the ring. Call once at boot, after NVS is up
 *  (the boot counter lives there).
 *
 *  Failure is NOT fatal: on any error this module disables itself and every
 *  other entry point becomes a no-op. A logging volume that will not mount must
 *  never take the fleet down with it. */
esp_err_t node_log_init(void);

/** True when the ring mounted and writes are being accepted. */
bool node_log_is_active(void);

/** Append the boot record: reset reason, the previous session's final uptime,
 *  and a health snapshot.
 *
 *  The highest-value record here. It catches a HANG FOLLOWED BY A WATCHDOG
 *  RESET, which is the failure that bites this fleet and the one a coredump
 *  does not capture. */
void node_log_boot(uint32_t reset_reason, uint32_t prev_uptime_s);

/** Append a periodic health sample. Rate-limited internally, so a caller cannot
 *  wear the flash by calling it in a tight loop. */
void node_log_periodic(void);

/** Append an event record. a/b carry subtype-specific detail -- for a WiFi
 *  disconnect they are the reason code and the RSSI at that moment, the pair
 *  main.c currently logs only to the console. */
void node_log_event(uint8_t subtype, int32_t a, int32_t b);

/** Record a WiFi disconnect for the health snapshot's cumulative counters.
 *  Kept as a setter so this module has no include cycle with main.c, which is
 *  where the reason and RSSI are actually known. */
void node_log_note_disconnect(uint8_t reason, int8_t rssi);

/** Copy raw records into out, newest-first, for the HTTP reader.
 *
 *  @param out         destination, at least max_records * NODE_LOG_RECORD_SIZE
 *  @param max_records maximum to copy
 *  @param skip        how many of the newest to skip, so a caller can page
 *  @return number of records written into out */
size_t node_log_read(uint8_t *out, size_t max_records, size_t skip);

/** Total records currently held in the ring. */
size_t node_log_count(void);

/** Boot id of the current session. Records from the same boot share it. */
uint16_t node_log_boot_id(void);

/** Erase every record. Deliberately explicit and never called automatically --
 *  the whole point of the module is that state survives. */
esp_err_t node_log_clear(void);

#ifdef __cplusplus
}
#endif
