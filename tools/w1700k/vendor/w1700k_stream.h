/* SPDX-License-Identifier: MIT */
/*
 * The W1700K's IQ stream as every consumer on the host reads it: the shared memory of
 * w1700k-stream (host/stream). One POSIX shared-memory object, by default /dev/shm/w1700k-stream,
 * mapped by the service and by each consumer. Native byte order and alignment; every word shared
 * between processes is read and written with the __atomic builtins, from C and C++ alike.
 *
 * Layout: the header (4 KiB), then one 64-bit tag per slot, then the samples: per slot one slice
 * of W1700K_STREAM_SLICE complex samples, int16 I then int16 Q, on the scale of the radio's 14-bit
 * samples (full scale W1700K_STREAM_FULL_SCALE). The int16 range above it is headroom for the
 * correction filter's gain; a sample beyond it saturates and is counted.
 *
 * Slot `index & (ring_slices - 1)` holds slice `index` of run `run` (a run is one stream, numbered
 * from 1; `index` counts the unit's DMA completions from the run's first, 0..). Its tag is
 * w1700k_stream_tag(run, index), with W1700K_STREAM_TAG_EVENT for a firmware event (no samples and
 * no sample time: the slices either side are contiguous), W1700K_STREAM_TAG_LOST for a completion
 * that never arrived (no samples; the sample time is lost), and W1700K_STREAM_TAG_FLAT on a slice
 * the correction filter produced. A reader reads the tag (acquire), the samples, then the tag
 * again: a changed tag means the service overwrote the slot meanwhile (a seqlock).
 *
 * The service publishes slots in index order. `newest` is the frontier: every slot of the run
 * below it is final. Read run and newest together with w1700k_stream_position().
 */
#ifndef W1700K_STREAM_H
#define W1700K_STREAM_H

#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <stdint.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

#define W1700K_STREAM_NAME		"/w1700k-stream"
#define W1700K_STREAM_MAGIC		0x4d525453u	/* "STRM" */
#define W1700K_STREAM_VERSION		1u
#define W1700K_STREAM_SLICE		2048u		/* complex samples per slot */
#define W1700K_STREAM_FULL_SCALE	8192		/* the radio's 14-bit full scale */
#define W1700K_STREAM_HEADER_BYTES	4096u

#define W1700K_STREAM_TAG_EVENT		(1ull << 63)
#define W1700K_STREAM_TAG_LOST		(1ull << 62)
#define W1700K_STREAM_TAG_FLAT		(1ull << 61)
#define W1700K_STREAM_TAG_KIND		(W1700K_STREAM_TAG_EVENT | W1700K_STREAM_TAG_LOST | W1700K_STREAM_TAG_FLAT)

struct w1700k_stream_header {
	/* layout, fixed at creation */
	uint32_t magic, version;
	uint32_t header_bytes, slice_samples;
	uint64_t ring_slices;		/* a power of two */
	uint64_t tags_offset, samples_offset, total_bytes;
	uint64_t service_pid;
	uint64_t heartbeat_ns;		/* CLOCK_MONOTONIC, refreshed about every 100 ms */
	uint64_t reserved0[8];

	/* the published position */
	uint64_t position_seq;		/* odd while the service writes run and newest */
	uint64_t run;			/* 1..; 0: no stream yet */
	uint64_t newest;		/* slots of the run below this are final */
	uint64_t run_started_ns, last_any_ns;	/* CLOCK_MONOTONIC */
	uint64_t received;		/* the newest completion received + 1 (ahead of newest) */
	uint64_t reserved1[10];

	/* counters since the service started */
	uint64_t datagrams, bytes;
	uint64_t slices, packed, events, lost;	/* completions filed: samples (packed among them), events, lost */
	uint64_t pieces, bad, late, before_start, duplicates, socket_drops;
	/* the export's integrity: raw slices with spare bits set (words), and slices or 32-byte lines
	 * equal to the same slice or line one lap (128 completions) earlier: a capture or packer
	 * that repeated old data */
	uint64_t spare_words, stale_slices, stale_lines;
	uint64_t flattened, saturated;	/* slices the correction produced; samples clipped to int16 */
	uint64_t overruns;		/* of the lost: arrived, but overwritten in the service's receive
					 * ring before it was published (the service fell behind) */
	uint64_t reserved2[12];

	/* control: written by consumers, read by the service */
	uint64_t restart_requests;	/* +1: the next completion starts a new run */
	uint64_t flatten;		/* 1: correct the decimation's roll-off where it applies */
	uint64_t radio_sample_rate;	/* samples/s; 0: unknown */
	uint64_t radio_capture_rate;	/* samples/s before the stream (640/2: 640e6); 0: unknown */
	uint64_t radio_filter_hz;	/* the analog filter's width; 0: unknown */
	uint64_t reserved3[11];

	/* the service's view of the control */
	uint64_t flatten_active;	/* the correction applies to the slices being published */
	uint64_t flatten_rate;		/* the sample rate whose correction applies; 0: none */
	uint64_t reserved4[14];
};

#define w1700k_stream_load(p)		__atomic_load_n((p), __ATOMIC_ACQUIRE)
#define w1700k_stream_store(p, v)	__atomic_store_n((p), (v), __ATOMIC_RELEASE)

static inline uint64_t w1700k_stream_tag(uint64_t run, uint64_t index)
{
	return (run << 40) | (index + 1);
}

static inline uint64_t *w1700k_stream_tags(const struct w1700k_stream_header *h)
{
	return (uint64_t *)((char *)h + h->tags_offset);
}

static inline int16_t *w1700k_stream_slice(const struct w1700k_stream_header *h, uint64_t index)
{
	return (int16_t *)((char *)h + h->samples_offset) +
	       (size_t)(index & (h->ring_slices - 1)) * W1700K_STREAM_SLICE * 2;
}

/* run and newest, read consistently. */
static inline void w1700k_stream_position(const struct w1700k_stream_header *h, uint64_t *run, uint64_t *newest)
{
	uint64_t s0, s1;

	do {
		s0 = w1700k_stream_load(&h->position_seq);
		*run = w1700k_stream_load(&h->run);
		*newest = w1700k_stream_load(&h->newest);
		s1 = w1700k_stream_load(&h->position_seq);
	} while ((s0 & 1) || s0 != s1);
}

static inline uint64_t w1700k_stream_now_ns(void)
{
	struct timespec t;

	clock_gettime(CLOCK_MONOTONIC, &t);
	return (uint64_t)t.tv_sec * 1000000000ull + (uint64_t)t.tv_nsec;
}

/* The service is up: it refreshed its heartbeat within the last second. */
static inline int w1700k_stream_alive(const struct w1700k_stream_header *h)
{
	return w1700k_stream_now_ns() - w1700k_stream_load(&h->heartbeat_ns) < 1000000000ull;
}

/*
 * Maps the service's shared memory (name NULL: W1700K_STREAM_NAME); `writable` for a consumer that
 * writes the control words. Returns NULL with errno set: ENOENT while no service has created it,
 * EPROTO for another layout. *bytes receives the mapping's length (for munmap), *inode the
 * object's inode (a restarted service creates a new one).
 */
static inline struct w1700k_stream_header *w1700k_stream_open(const char *name, int writable, size_t *bytes,
							      uint64_t *inode)
{
	struct w1700k_stream_header *h;
	struct stat st;
	int fd = shm_open(name ? name : W1700K_STREAM_NAME, writable ? O_RDWR : O_RDONLY, 0);

	if (fd < 0)
		return NULL;
	if (fstat(fd, &st) || (size_t)st.st_size < W1700K_STREAM_HEADER_BYTES) {
		close(fd);
		errno = EPROTO;
		return NULL;
	}
	h = (struct w1700k_stream_header *)mmap(NULL, (size_t)st.st_size,
						writable ? PROT_READ | PROT_WRITE : PROT_READ, MAP_SHARED, fd, 0);
	close(fd);
	if (h == MAP_FAILED)
		return NULL;
	if (h->magic != W1700K_STREAM_MAGIC || h->version != W1700K_STREAM_VERSION ||
	    h->slice_samples != W1700K_STREAM_SLICE || h->total_bytes != (uint64_t)st.st_size) {
		munmap(h, (size_t)st.st_size);
		errno = EPROTO;
		return NULL;
	}
	*bytes = (size_t)st.st_size;
	if (inode)
		*inode = (uint64_t)st.st_ino;
	return h;
}

#endif /* W1700K_STREAM_H */
