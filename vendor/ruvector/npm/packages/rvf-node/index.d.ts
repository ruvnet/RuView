/* auto-generated: TypeScript declarations for @ruvector/rvf-node */

export interface RvfOptions {
  dimension: number;
  metric?: string;
  profile?: number;
  signing?: boolean;
  m?: number;
  efConstruction?: number;
}

export interface RvfQueryOptions {
  efSearch?: number;
  filter?: string;
  timeoutMs?: number;
}

export interface RvfSearchResult {
  id: number;
  distance: number;
}

export interface RvfIngestResult {
  accepted: number;
  rejected: number;
  epoch: number;
}

export interface RvfDeleteResult {
  deleted: number;
  epoch: number;
}

export interface RvfCompactionResult {
  segmentsCompacted: number;
  bytesReclaimed: number;
  epoch: number;
}

export interface RvfStatus {
  totalVectors: number;
  totalSegments: number;
  fileSize: number;
  currentEpoch: number;
  profileId: number;
  compactionState: string;
  deadSpaceRatio: number;
  readOnly: boolean;
}

export interface RvfMetadataEntry {
  fieldId: number;
  valueType: string;
  value: string;
}

export interface RvfKernelData {
  header: Buffer;
  image: Buffer;
}

export interface RvfEbpfData {
  header: Buffer;
  payload: Buffer;
}

export interface RvfSegmentInfo {
  id: number;
  offset: number;
  payloadLength: number;
  segType: string;
}

export class RvfDatabase {
  static create(path: string, options: RvfOptions): RvfDatabase;
  static open(path: string): RvfDatabase;
  static openReadonly(path: string): RvfDatabase;
  ingestBatch(vectors: Float32Array, ids: number[], metadata?: RvfMetadataEntry[]): RvfIngestResult;
  query(vector: Float32Array, k: number, options?: RvfQueryOptions): RvfSearchResult[];
  delete(ids: number[]): RvfDeleteResult;
  deleteByFilter(filterJson: string): RvfDeleteResult;
  compact(): RvfCompactionResult;
  status(): RvfStatus;
  close(): void;
  fileId(): string;
  parentId(): string;
  lineageDepth(): number;
  derive(childPath: string, options?: RvfOptions): RvfDatabase;
  embedKernel(arch: number, kernelType: number, flags: number, image: Buffer, apiPort: number, cmdline?: string): number;
  extractKernel(): RvfKernelData | null;
  embedEbpf(programType: number, attachType: number, maxDimension: number, bytecode: Buffer, btf?: Buffer): number;
  extractEbpf(): RvfEbpfData | null;
  segments(): RvfSegmentInfo[];
  dimension(): number;
}
