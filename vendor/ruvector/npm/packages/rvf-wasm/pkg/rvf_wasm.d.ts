/**
 * Type declarations for the RVF WASM microkernel exports.
 */

export interface RvfWasmExports {
  memory: WebAssembly.Memory;

  // Memory management
  rvf_alloc(size: number): number;
  rvf_free(ptr: number, size: number): void;

  // Core query path
  rvf_init(config_ptr: number): number;
  rvf_load_query(query_ptr: number, dim: number): number;
  rvf_load_block(block_ptr: number, count: number, dtype: number): number;
  rvf_distances(metric: number, result_ptr: number): number;
  rvf_topk_merge(dist_ptr: number, id_ptr: number, count: number, k: number): number;
  rvf_topk_read(out_ptr: number): number;

  // Quantization
  rvf_load_sq_params(params_ptr: number, dim: number): number;
  rvf_dequant_i8(src_ptr: number, dst_ptr: number, count: number): number;
  rvf_load_pq_codebook(codebook_ptr: number, m: number, k: number): number;
  rvf_pq_distances(codes_ptr: number, count: number, result_ptr: number): number;

  // HNSW navigation
  rvf_load_neighbors(node_id: bigint, layer: number, out_ptr: number): number;
  rvf_greedy_step(current_id: bigint, layer: number): bigint;

  // Segment verification
  rvf_verify_header(header_ptr: number): number;
  rvf_crc32c(data_ptr: number, len: number): number;
  rvf_verify_checksum(buf_ptr: number, buf_len: number): number;

  // In-memory store
  rvf_store_create(dim: number, metric: number): number;
  rvf_store_open(buf_ptr: number, buf_len: number): number;
  rvf_store_ingest(handle: number, vecs_ptr: number, ids_ptr: number, count: number): number;
  rvf_store_query(handle: number, query_ptr: number, k: number, metric: number, out_ptr: number): number;
  rvf_store_delete(handle: number, ids_ptr: number, count: number): number;
  rvf_store_count(handle: number): number;
  rvf_store_dimension(handle: number): number;
  rvf_store_status(handle: number, out_ptr: number): number;
  rvf_store_export(handle: number, out_ptr: number, out_len: number): number;
  rvf_store_close(handle: number): number;

  // Segment parsing
  rvf_parse_header(buf_ptr: number, buf_len: number, out_ptr: number): number;
  rvf_segment_count(buf_ptr: number, buf_len: number): number;
  rvf_segment_info(buf_ptr: number, buf_len: number, idx: number, out_ptr: number): number;

  // Witness chain
  rvf_witness_verify(chain_ptr: number, chain_len: number): number;
  rvf_witness_count(chain_len: number): number;
}

export default function init(input?: ArrayBuffer | Uint8Array | WebAssembly.Module): Promise<RvfWasmExports>;
