/**
 * @ruvector/scipix - OCR Client for Scientific Documents
 *
 * A TypeScript client for the SciPix OCR API, enabling extraction of
 * LaTeX, MathML, and text from scientific images, equations, and documents.
 *
 * @example
 * ```typescript
 * import { SciPixClient, OutputFormat } from '@ruvector/scipix';
 *
 * // Create client
 * const client = new SciPixClient({
 *   baseUrl: 'http://localhost:8080',
 *   apiKey: 'your-api-key',
 * });
 *
 * // OCR an image file
 * const result = await client.ocrFile('./equation.png', {
 *   formats: [OutputFormat.LaTeX, OutputFormat.MathML],
 *   detectEquations: true,
 * });
 *
 * console.log('LaTeX:', result.latex);
 * console.log('Confidence:', result.confidence);
 *
 * // Quick LaTeX extraction
 * const latex = await client.extractLatex('./math.png');
 * console.log('Extracted LaTeX:', latex);
 *
 * // Batch processing
 * const batchResult = await client.batchOcr({
 *   images: [
 *     { source: 'base64...', id: 'eq1' },
 *     { source: 'base64...', id: 'eq2' },
 *   ],
 *   defaultOptions: { formats: [OutputFormat.LaTeX] },
 * });
 *
 * console.log(`Processed ${batchResult.successful}/${batchResult.totalImages} images`);
 * ```
 *
 * @packageDocumentation
 */

// Types
export {
  OutputFormat,
  ImageType,
  ConfidenceLevel,
  ContentType,
  BoundingBox,
  OCRRegion,
  OCRResult,
  OCROptions,
  BatchOCRRequest,
  BatchOCRResult,
  SciPixConfig,
  HealthStatus,
  SciPixError,
  SciPixErrorCode,
} from './types.js';

// Client
export { SciPixClient, createClient } from './client.js';
