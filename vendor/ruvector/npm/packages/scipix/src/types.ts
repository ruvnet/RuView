/**
 * SciPix OCR Types
 * Types for scientific document OCR and equation recognition
 */

/** Supported output formats for OCR results */
export enum OutputFormat {
  /** LaTeX mathematical notation */
  LaTeX = 'latex',
  /** MathML markup language */
  MathML = 'mathml',
  /** ASCII math notation */
  AsciiMath = 'asciimath',
  /** Plain text */
  Text = 'text',
  /** Structured JSON with metadata */
  JSON = 'json',
}

/** Supported image input types */
export enum ImageType {
  PNG = 'png',
  JPEG = 'jpeg',
  WebP = 'webp',
  PDF = 'pdf',
  TIFF = 'tiff',
  BMP = 'bmp',
}

/** OCR confidence level */
export enum ConfidenceLevel {
  High = 'high',
  Medium = 'medium',
  Low = 'low',
}

/** Type of content detected in the image */
export enum ContentType {
  /** Mathematical equation */
  Equation = 'equation',
  /** Text content */
  Text = 'text',
  /** Table structure */
  Table = 'table',
  /** Diagram or chart */
  Diagram = 'diagram',
  /** Mixed content */
  Mixed = 'mixed',
}

/** Bounding box for detected regions */
export interface BoundingBox {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Single OCR result region */
export interface OCRRegion {
  /** Unique identifier for this region */
  id: string;
  /** Bounding box of the detected region */
  bbox: BoundingBox;
  /** Type of content detected */
  contentType: ContentType;
  /** Raw text content */
  text: string;
  /** LaTeX representation (if applicable) */
  latex?: string;
  /** MathML representation (if applicable) */
  mathml?: string;
  /** Confidence score (0-1) */
  confidence: number;
  /** Confidence level */
  confidenceLevel: ConfidenceLevel;
}

/** Complete OCR result */
export interface OCRResult {
  /** Unique result identifier */
  id: string;
  /** Original image dimensions */
  imageDimensions: {
    width: number;
    height: number;
  };
  /** All detected regions */
  regions: OCRRegion[];
  /** Combined text output */
  text: string;
  /** Combined LaTeX output (if requested) */
  latex?: string;
  /** Combined MathML output (if requested) */
  mathml?: string;
  /** Processing time in milliseconds */
  processingTime: number;
  /** Model version used */
  modelVersion: string;
  /** Overall confidence */
  confidence: number;
  /** Metadata */
  metadata: {
    imageType: ImageType;
    hasEquations: boolean;
    hasTables: boolean;
    hasDiagrams: boolean;
    pageCount?: number;
  };
}

/** OCR request options */
export interface OCROptions {
  /** Desired output formats */
  formats?: OutputFormat[];
  /** Language hints for OCR */
  languages?: string[];
  /** Enable equation detection */
  detectEquations?: boolean;
  /** Enable table detection */
  detectTables?: boolean;
  /** Enable diagram detection */
  detectDiagrams?: boolean;
  /** Minimum confidence threshold (0-1) */
  minConfidence?: number;
  /** Enable preprocessing (deskew, denoise) */
  preprocess?: boolean;
  /** DPI hint for scanned documents */
  dpi?: number;
  /** Specific pages to process (for PDFs) */
  pages?: number[];
}

/** Batch OCR request */
export interface BatchOCRRequest {
  /** Array of image URLs or base64 data */
  images: Array<{
    /** URL or base64 data */
    source: string;
    /** Optional identifier */
    id?: string;
    /** Per-image options */
    options?: OCROptions;
  }>;
  /** Default options for all images */
  defaultOptions?: OCROptions;
}

/** Batch OCR result */
export interface BatchOCRResult {
  /** Total images processed */
  totalImages: number;
  /** Successful results */
  successful: number;
  /** Failed results */
  failed: number;
  /** Individual results */
  results: Array<{
    id: string;
    success: boolean;
    result?: OCRResult;
    error?: string;
  }>;
  /** Total processing time */
  totalProcessingTime: number;
}

/** SciPix client configuration */
export interface SciPixConfig {
  /** API base URL */
  baseUrl?: string;
  /** API key for authentication */
  apiKey?: string;
  /** Request timeout in milliseconds */
  timeout?: number;
  /** Maximum retries for failed requests */
  maxRetries?: number;
  /** Default OCR options */
  defaultOptions?: OCROptions;
}

/** Health check response */
export interface HealthStatus {
  status: 'healthy' | 'degraded' | 'unhealthy';
  version: string;
  models: {
    name: string;
    loaded: boolean;
    version: string;
  }[];
  uptime: number;
}

/** Error types */
export class SciPixError extends Error {
  constructor(
    message: string,
    public readonly code: SciPixErrorCode,
    public readonly statusCode?: number,
  ) {
    super(message);
    this.name = 'SciPixError';
  }

  static networkError(message: string): SciPixError {
    return new SciPixError(message, SciPixErrorCode.Network);
  }

  static serverError(message: string, statusCode: number): SciPixError {
    return new SciPixError(message, SciPixErrorCode.Server, statusCode);
  }

  static invalidImage(message: string): SciPixError {
    return new SciPixError(message, SciPixErrorCode.InvalidImage);
  }

  static timeout(): SciPixError {
    return new SciPixError('Request timed out', SciPixErrorCode.Timeout);
  }
}

export enum SciPixErrorCode {
  Network = 'NETWORK',
  Server = 'SERVER',
  InvalidImage = 'INVALID_IMAGE',
  Timeout = 'TIMEOUT',
  InvalidConfig = 'INVALID_CONFIG',
  Unauthorized = 'UNAUTHORIZED',
  RateLimited = 'RATE_LIMITED',
}
