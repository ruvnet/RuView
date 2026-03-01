/**
 * SciPix OCR Client
 * Client for interacting with SciPix OCR API
 */

import { readFile } from 'node:fs/promises';
import { extname } from 'node:path';
import {
  type SciPixConfig,
  type OCROptions,
  type OCRResult,
  type BatchOCRRequest,
  type BatchOCRResult,
  type HealthStatus,
  SciPixError,
  SciPixErrorCode,
  OutputFormat,
  ImageType,
} from './types.js';

/** Default configuration */
const DEFAULT_CONFIG: Required<Omit<SciPixConfig, 'apiKey'>> = {
  baseUrl: 'http://localhost:8080',
  timeout: 30000,
  maxRetries: 3,
  defaultOptions: {
    formats: [OutputFormat.LaTeX, OutputFormat.Text],
    detectEquations: true,
    preprocess: true,
  },
};

/** SciPix OCR Client */
export class SciPixClient {
  private config: Required<Omit<SciPixConfig, 'apiKey'>> & { apiKey?: string };

  constructor(config?: SciPixConfig) {
    this.config = { ...DEFAULT_CONFIG, ...config };
  }

  /**
   * Perform OCR on an image
   * @param image - Image data as Buffer, base64 string, or file path
   * @param options - OCR options
   */
  async ocr(image: Buffer | string, options?: OCROptions): Promise<OCRResult> {
    const imageData = await this.prepareImage(image);
    const mergedOptions = { ...this.config.defaultOptions, ...options };

    const response = await this.request('/api/v1/ocr', {
      method: 'POST',
      body: JSON.stringify({
        image: imageData.base64,
        imageType: imageData.type,
        options: mergedOptions,
      }),
    });

    return response as OCRResult;
  }

  /**
   * Perform OCR on a file
   * @param filePath - Path to the image file
   * @param options - OCR options
   */
  async ocrFile(filePath: string, options?: OCROptions): Promise<OCRResult> {
    const buffer = await readFile(filePath);
    return this.ocr(buffer, options);
  }

  /**
   * Perform batch OCR on multiple images
   * @param request - Batch OCR request
   */
  async batchOcr(request: BatchOCRRequest): Promise<BatchOCRResult> {
    const response = await this.request('/api/v1/ocr/batch', {
      method: 'POST',
      body: JSON.stringify(request),
    });

    return response as BatchOCRResult;
  }

  /**
   * Extract LaTeX from an equation image
   * @param image - Image data
   */
  async extractLatex(image: Buffer | string): Promise<string> {
    const result = await this.ocr(image, {
      formats: [OutputFormat.LaTeX],
      detectEquations: true,
    });

    return result.latex ?? result.text;
  }

  /**
   * Extract MathML from an equation image
   * @param image - Image data
   */
  async extractMathML(image: Buffer | string): Promise<string> {
    const result = await this.ocr(image, {
      formats: [OutputFormat.MathML],
      detectEquations: true,
    });

    return result.mathml ?? '';
  }

  /**
   * Check API health status
   */
  async health(): Promise<HealthStatus> {
    const response = await this.request('/api/v1/health', {
      method: 'GET',
    });

    return response as HealthStatus;
  }

  /**
   * Prepare image for API request
   */
  private async prepareImage(
    image: Buffer | string,
  ): Promise<{ base64: string; type: ImageType }> {
    let buffer: Buffer;
    let type: ImageType = ImageType.PNG;

    if (Buffer.isBuffer(image)) {
      buffer = image;
      type = this.detectImageType(buffer);
    } else if (image.startsWith('data:')) {
      // Base64 data URL
      const match = image.match(/^data:image\/(\w+);base64,(.+)$/);
      if (!match) {
        throw SciPixError.invalidImage('Invalid data URL format');
      }
      type = this.parseImageType(match[1]);
      return { base64: match[2], type };
    } else if (image.startsWith('/') || image.includes(':\\')) {
      // File path
      buffer = await readFile(image);
      type = this.getTypeFromExtension(extname(image));
    } else {
      // Assume base64 string
      return { base64: image, type: ImageType.PNG };
    }

    return {
      base64: buffer.toString('base64'),
      type,
    };
  }

  /**
   * Detect image type from buffer magic bytes
   */
  private detectImageType(buffer: Buffer): ImageType {
    if (buffer[0] === 0x89 && buffer[1] === 0x50) return ImageType.PNG;
    if (buffer[0] === 0xff && buffer[1] === 0xd8) return ImageType.JPEG;
    if (buffer[0] === 0x52 && buffer[1] === 0x49) return ImageType.WebP;
    if (buffer[0] === 0x25 && buffer[1] === 0x50) return ImageType.PDF;
    if (buffer[0] === 0x49 && buffer[1] === 0x49) return ImageType.TIFF;
    if (buffer[0] === 0x4d && buffer[1] === 0x4d) return ImageType.TIFF;
    if (buffer[0] === 0x42 && buffer[1] === 0x4d) return ImageType.BMP;
    return ImageType.PNG; // Default
  }

  /**
   * Parse image type from MIME type
   */
  private parseImageType(mimeType: string): ImageType {
    switch (mimeType.toLowerCase()) {
      case 'png':
        return ImageType.PNG;
      case 'jpeg':
      case 'jpg':
        return ImageType.JPEG;
      case 'webp':
        return ImageType.WebP;
      case 'pdf':
        return ImageType.PDF;
      case 'tiff':
      case 'tif':
        return ImageType.TIFF;
      case 'bmp':
        return ImageType.BMP;
      default:
        return ImageType.PNG;
    }
  }

  /**
   * Get image type from file extension
   */
  private getTypeFromExtension(ext: string): ImageType {
    return this.parseImageType(ext.slice(1));
  }

  /**
   * Make HTTP request to API
   */
  private async request(
    path: string,
    options: RequestInit,
    retries = 0,
  ): Promise<unknown> {
    const url = `${this.config.baseUrl}${path}`;
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    };

    if (this.config.apiKey) {
      headers['Authorization'] = `Bearer ${this.config.apiKey}`;
    }

    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.config.timeout);

    try {
      const response = await fetch(url, {
        ...options,
        headers: { ...headers, ...options.headers },
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

      if (!response.ok) {
        const error = await response.text();

        if (response.status === 401) {
          throw new SciPixError('Unauthorized', SciPixErrorCode.Unauthorized, 401);
        }
        if (response.status === 429) {
          throw new SciPixError('Rate limited', SciPixErrorCode.RateLimited, 429);
        }

        throw SciPixError.serverError(error, response.status);
      }

      return await response.json();
    } catch (error) {
      clearTimeout(timeoutId);

      if (error instanceof SciPixError) {
        throw error;
      }

      if ((error as Error).name === 'AbortError') {
        throw SciPixError.timeout();
      }

      // Retry on network errors
      if (retries < this.config.maxRetries) {
        await new Promise((resolve) => setTimeout(resolve, 1000 * (retries + 1)));
        return this.request(path, options, retries + 1);
      }

      throw SciPixError.networkError((error as Error).message);
    }
  }
}

/**
 * Create a SciPix client with default configuration
 */
export function createClient(config?: SciPixConfig): SciPixClient {
  return new SciPixClient(config);
}
