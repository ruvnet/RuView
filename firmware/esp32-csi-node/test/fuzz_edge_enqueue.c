/**
 * @file fuzz_edge_enqueue.c
 * @brief libFuzzer target for edge_enqueue_csi() (ADR-061 Layer 6).
 *
 * Rapid-fire enqueues with varying iq_len from 0 to beyond
 * EDGE_MAX_IQ_BYTES, testing the SPSC ring buffer overflow behavior
 * and verifying no out-of-bounds writes occur.
 *
 * Build (Linux/macOS with clang):
 *   make fuzz_edge
 *
 * Run:
 *   ./fuzz_edge corpus/ -max_len=4096
 */

#include "esp_stubs.h"

/*
 * We cannot include edge_processing.c directly because it references
 * FreeRTOS task creation and other ESP-IDF APIs in edge_processing_init().
 * Instead, we re-implement the SPSC ring buffer and edge_enqueue_csi()
 * logic identically to the production code, testing the same algorithm.
 */

#include <stdint.h>
#include <stddef.h>
#include <string.h>
#include <stdlib.h>

/* ---- Reproduce the ring buffer from edge_processing.h ----
 *
 * EDGE_MAX_SUBCARRIERS deliberately does NOT appear here. This target
 * exercises the SPSC ring (ring_push/ring_pop), which is bounded by
 * EDGE_MAX_IQ_BYTES and never consults the subcarrier grid. It previously
 * carried a private `#define EDGE_MAX_SUBCARRIERS 128` that nothing read --
 * dead, and free to drift from the real constant without any test failing.
 * That is precisely how the HE20 truncation bug survived: the grid is
 * target-conditional (128 pre-HE, 256 on C6/C5) and a stale private copy
 * would have masked it.
 *
 * The grid is pinned against the real header in test_edge_subcarrier_grid.c.
 */
#define EDGE_RING_SLOTS       16
#define EDGE_MAX_IQ_BYTES     1024

typedef struct {
    uint8_t  iq_data[EDGE_MAX_IQ_BYTES];
    uint16_t iq_len;
    int8_t   rssi;
    uint8_t  channel;
    uint32_t timestamp_us;
} fuzz_ring_slot_t;

typedef struct {
    fuzz_ring_slot_t slots[EDGE_RING_SLOTS];
    volatile uint32_t head;
    volatile uint32_t tail;
} fuzz_ring_buf_t;

static fuzz_ring_buf_t s_ring;

/**
 * ring_push: identical logic to edge_processing.c::ring_push().
 * This is the code path exercised by edge_enqueue_csi().
 */
static bool ring_push(const uint8_t *iq, uint16_t len,
                       int8_t rssi, uint8_t channel)
{
    uint32_t next = (s_ring.head + 1) % EDGE_RING_SLOTS;
    if (next == s_ring.tail) {
        return false;  /* Full. */
    }

    fuzz_ring_slot_t *slot = &s_ring.slots[s_ring.head];
    uint16_t copy_len = (len > EDGE_MAX_IQ_BYTES) ? EDGE_MAX_IQ_BYTES : len;
    memcpy(slot->iq_data, iq, copy_len);
    slot->iq_len = copy_len;
    slot->rssi = rssi;
    slot->channel = channel;
    slot->timestamp_us = (uint32_t)(esp_timer_get_time() & 0xFFFFFFFF);

    __sync_synchronize();
    s_ring.head = next;
    return true;
}

/**
 * ring_pop: identical logic to edge_processing.c::ring_pop().
 */
static bool ring_pop(fuzz_ring_slot_t *out)
{
    if (s_ring.tail == s_ring.head) {
        return false;
    }

    memcpy(out, &s_ring.slots[s_ring.tail], sizeof(fuzz_ring_slot_t));

    __sync_synchronize();
    s_ring.tail = (s_ring.tail + 1) % EDGE_RING_SLOTS;
    return true;
}

/**
 * Canary pattern: write to a buffer zone after ring memory to detect
 * out-of-bounds writes. If the canary is overwritten, we trap.
 */
#define CANARY_SIZE  64
#define CANARY_BYTE  0xCD
static uint8_t s_canary_before[CANARY_SIZE];
/* s_ring is between the canaries (static allocation order not guaranteed,
 * but ASAN will catch OOB writes regardless). */
static uint8_t s_canary_after[CANARY_SIZE];

static void init_canaries(void)
{
    memset(s_canary_before, CANARY_BYTE, CANARY_SIZE);
    memset(s_canary_after, CANARY_BYTE, CANARY_SIZE);
}

static void check_canaries(void)
{
    for (int i = 0; i < CANARY_SIZE; i++) {
        if (s_canary_before[i] != CANARY_BYTE) __builtin_trap();
        if (s_canary_after[i] != CANARY_BYTE) __builtin_trap();
    }
}

int LLVMFuzzerTestOneInput(const uint8_t *data, size_t size)
{
    if (size < 4) return 0;

    /* Reset ring buffer state for each fuzz iteration. */
    memset(&s_ring, 0, sizeof(s_ring));
    init_canaries();

    const uint8_t *cursor = data;
    size_t remaining = size;

    /*
     * Protocol: each "enqueue command" is:
     *   [0..1] iq_len (LE u16)
     *   [2]    rssi (i8)
     *   [3]    channel (u8)
     *   [4..]  iq_data (up to iq_len bytes, zero-padded if short)
     *
     * We consume commands until data is exhausted.
     */
    uint32_t enqueue_count = 0;
    uint32_t full_count = 0;
    uint32_t pop_count = 0;

    while (remaining >= 4) {
        uint16_t iq_len = (uint16_t)cursor[0] | ((uint16_t)cursor[1] << 8);
        int8_t   rssi   = (int8_t)cursor[2];
        uint8_t  channel = cursor[3];
        cursor += 4;
        remaining -= 4;

        /* Prepare I/Q data buffer.
         * Even if iq_len > EDGE_MAX_IQ_BYTES, we pass it to ring_push
         * which must clamp it internally. We need a source buffer that
         * is at least iq_len bytes to avoid reading OOB. */
        uint8_t iq_buf[EDGE_MAX_IQ_BYTES + 128];
        memset(iq_buf, 0, sizeof(iq_buf));

        /* Copy available fuzz data into iq_buf. */
        uint16_t avail = (remaining > sizeof(iq_buf))
                         ? (uint16_t)sizeof(iq_buf)
                         : (uint16_t)remaining;
        if (avail > 0) {
            memcpy(iq_buf, cursor, avail);
        }

        /* Advance cursor past the I/Q data portion.
         * We consume min(iq_len, remaining) bytes. */
        uint16_t consume = (iq_len > remaining) ? (uint16_t)remaining : iq_len;
        cursor += consume;
        remaining -= consume;

        /* The key test: iq_len can be 0, normal, EDGE_MAX_IQ_BYTES,
         * or larger (up to 65535). ring_push must clamp to EDGE_MAX_IQ_BYTES. */
        bool ok = ring_push(iq_buf, iq_len, rssi, channel);
        if (ok) {
            enqueue_count++;
        } else {
            full_count++;

            /* When ring is full, drain one slot to make room.
             * This tests the interleaved push/pop pattern. */
            fuzz_ring_slot_t popped;
            if (ring_pop(&popped)) {
                pop_count++;

                /* Verify popped data is sane. */
                if (popped.iq_len > EDGE_MAX_IQ_BYTES) {
                    __builtin_trap();  /* Clamping failed. */
                }
            }

            /* Retry the enqueue after popping. */
            ring_push(iq_buf, iq_len, rssi, channel);
        }

        /* Periodically check canaries. */
        if ((enqueue_count + full_count) % 8 == 0) {
            check_canaries();
        }
    }

    /* Drain remaining items and verify each. */
    fuzz_ring_slot_t popped;
    while (ring_pop(&popped)) {
        pop_count++;
        if (popped.iq_len > EDGE_MAX_IQ_BYTES) {
            __builtin_trap();
        }
    }

    /* Final canary check. */
    check_canaries();

    /* Verify ring is now empty. */
    if (s_ring.head != s_ring.tail) {
        __builtin_trap();
    }

    return 0;
}
