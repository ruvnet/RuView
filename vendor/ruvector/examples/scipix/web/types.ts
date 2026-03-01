/**
 * TypeScript definitions for Mathpix WASM module
 */

/**
 * OCR recognition result
 */
export interface OcrResult {
  /** Recognized plain text */
  text: string;

  /** LaTeX representation (if applicable) */
  latex?: string;

  /** Confidence score (0.0 - 1.0) */
  confidence: number;

  /** Additional metadata */
  metadata?: {
    width?: number;
    height?: number;
    format?: string;
    [key: string]: any;
  };
}

/**
 * Recognition output format
 */
export type RecognitionFormat = 'text' | 'latex' | 'both';

/**
 * Processing options
 */
export interface ProcessingOptions {
  /** Output format */
  format?: RecognitionFormat;

  /** Confidence threshold (0.0 - 1.0) */
  confidenceThreshold?: number;

  /** Enable preprocessing */
  preprocess?: boolean;

  /** Enable postprocessing */
  postprocess?: boolean;
}

/**
 * Main Mathpix WASM API
 */
export class MathpixWasm {
  /**
   * Create a new MathpixWasm instance
   */
  constructor();

  /**
   * Initialize and create a new instance
   */
  static new(): Promise<MathpixWasm>;

  /**
   * Recognize text from raw image data
   * @param imageData Raw image bytes (PNG, JPEG, etc.)
   * @returns OCR result
   */
  recognize(imageData: Uint8Array): Promise<OcrResult>;

  /**
   * Recognize text from HTML Canvas element
   * @param canvas HTML Canvas element
   * @returns OCR result
   */
  recognizeFromCanvas(canvas: HTMLCanvasElement): Promise<OcrResult>;

  /**
   * Recognize text from base64-encoded image
   * @param base64 Base64 string (with or without data URL prefix)
   * @returns OCR result
   */
  recognizeBase64(base64: string): Promise<OcrResult>;

  /**
   * Recognize text from ImageData object
   * @param imageData ImageData from canvas
   * @returns OCR result
   */
  recognizeImageData(imageData: ImageData): Promise<OcrResult>;

  /**
   * Set the output format
   * @param format Recognition format ('text', 'latex', or 'both')
   */
  setFormat(format: RecognitionFormat): void;

  /**
   * Set the confidence threshold
   * @param threshold Threshold value (0.0 - 1.0)
   */
  setConfidenceThreshold(threshold: number): void;

  /**
   * Get the current confidence threshold
   * @returns Current threshold value
   */
  getConfidenceThreshold(): number;

  /**
   * Get the library version
   * @returns Version string
   */
  getVersion(): string;

  /**
   * Get supported output formats
   * @returns Array of supported format strings
   */
  getSupportedFormats(): string[];

  /**
   * Batch process multiple images
   * @param images Array of image data (Uint8Array)
   * @returns Array of OCR results
   */
  recognizeBatch(images: Uint8Array[]): Promise<OcrResult[]>;
}

/**
 * Factory function to create MathpixWasm instance
 */
export function createMathpix(): Promise<MathpixWasm>;

/**
 * Get WASM module version
 */
export function version(): string;

/**
 * Check if WASM module is ready
 */
export function isReady(): boolean;

/**
 * Shared image buffer for efficient memory management
 */
export class SharedImageBuffer {
  /**
   * Create a new shared buffer
   * @param width Image width
   * @param height Image height
   */
  constructor(width: number, height: number);

  /** Image width */
  readonly width: number;

  /** Image height */
  readonly height: number;

  /** Buffer size in bytes */
  bufferSize(): number;

  /** Get buffer as Uint8Array */
  getBuffer(): Uint8Array;

  /** Set buffer from Uint8Array */
  setBuffer(data: Uint8Array): void;

  /** Clear the buffer */
  clear(): void;
}

/**
 * Convert blob URL to ImageData
 * @param blobUrl Blob URL string
 * @returns ImageData object
 */
export function blobUrlToImageData(blobUrl: string): Promise<ImageData>;

/**
 * Get memory usage statistics
 * @returns Memory stats object
 */
export function getMemoryStats(): any;

/**
 * Force garbage collection (hint to runtime)
 */
export function forceGC(): void;

/**
 * Worker message types
 */
export type WorkerRequestType =
  | 'Init'
  | 'Process'
  | 'ProcessBase64'
  | 'BatchProcess'
  | 'Terminate';

export type WorkerResponseType =
  | 'Ready'
  | 'Started'
  | 'Progress'
  | 'Success'
  | 'Error'
  | 'Terminated';

export interface WorkerRequest {
  type: WorkerRequestType;
  id?: string;
  imageData?: Uint8Array;
  base64?: string;
  images?: Uint8Array[];
  format?: RecognitionFormat;
}

export interface WorkerResponse {
  type: WorkerResponseType;
  id?: string;
  result?: OcrResult | OcrResult[];
  error?: string;
  processed?: number;
  total?: number;
}
