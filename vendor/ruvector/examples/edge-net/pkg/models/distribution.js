/**
 * @ruvector/edge-net Distribution Manager
 *
 * Handles model distribution across multiple sources:
 * - Google Cloud Storage (GCS)
 * - IPFS (via web3.storage or nft.storage)
 * - CDN with fallback support
 *
 * Features:
 * - Integrity verification (SHA256)
 * - Progress tracking for large files
 * - Automatic source failover
 *
 * @module @ruvector/edge-net/models/distribution
 */

import { EventEmitter } from 'events';
import { createHash, randomBytes } from 'crypto';
import { promises as fs } from 'fs';
import path from 'path';
import https from 'https';
import http from 'http';
import { URL } from 'url';

// ============================================
// CONSTANTS
// ============================================

const DEFAULT_GCS_BUCKET = 'ruvector-models';
const DEFAULT_CDN_BASE = 'https://models.ruvector.dev';
const DEFAULT_IPFS_GATEWAY = 'https://w3s.link/ipfs';

const CHUNK_SIZE = 1024 * 1024; // 1MB chunks for streaming
const MAX_RETRIES = 3;
const RETRY_DELAY_MS = 1000;

// ============================================
// SOURCE TYPES
// ============================================

/**
 * Source priority order (lower = higher priority)
 */
export const SOURCE_PRIORITY = {
    cdn: 1,
    gcs: 2,
    ipfs: 3,
    fallback: 99,
};

/**
 * Source URL patterns
 */
export const SOURCE_PATTERNS = {
    gcs: /^gs:\/\/([^/]+)\/(.+)$/,
    ipfs: /^ipfs:\/\/(.+)$/,
    http: /^https?:\/\/.+$/,
};

// ============================================
// PROGRESS TRACKER
// ============================================

/**
 * Progress tracker for file transfers
 */
export class ProgressTracker extends EventEmitter {
    constructor(totalBytes = 0) {
        super();
        this.totalBytes = totalBytes;
        this.bytesTransferred = 0;
        this.startTime = Date.now();
        this.lastUpdateTime = Date.now();
        this.lastBytesTransferred = 0;
    }

    /**
     * Update progress
     * @param {number} bytes - Bytes transferred in this chunk
     */
    update(bytes) {
        this.bytesTransferred += bytes;
        const now = Date.now();

        // Calculate speed (bytes per second)
        const timeDelta = (now - this.lastUpdateTime) / 1000;
        const bytesDelta = this.bytesTransferred - this.lastBytesTransferred;
        const speed = timeDelta > 0 ? bytesDelta / timeDelta : 0;

        // Calculate ETA
        const remaining = this.totalBytes - this.bytesTransferred;
        const eta = speed > 0 ? remaining / speed : 0;

        const progress = {
            bytesTransferred: this.bytesTransferred,
            totalBytes: this.totalBytes,
            percent: this.totalBytes > 0
                ? Math.round((this.bytesTransferred / this.totalBytes) * 100)
                : 0,
            speed: Math.round(speed),
            speedMBps: Math.round(speed / (1024 * 1024) * 100) / 100,
            eta: Math.round(eta),
            elapsed: Math.round((now - this.startTime) / 1000),
        };

        this.lastUpdateTime = now;
        this.lastBytesTransferred = this.bytesTransferred;

        this.emit('progress', progress);

        if (this.bytesTransferred >= this.totalBytes) {
            this.emit('complete', progress);
        }
    }

    /**
     * Mark as complete
     */
    complete() {
        this.bytesTransferred = this.totalBytes;
        const elapsed = (Date.now() - this.startTime) / 1000;

        this.emit('complete', {
            bytesTransferred: this.bytesTransferred,
            totalBytes: this.totalBytes,
            percent: 100,
            elapsed: Math.round(elapsed),
            averageSpeed: Math.round(this.totalBytes / elapsed),
        });
    }

    /**
     * Mark as failed
     * @param {Error} error - Failure error
     */
    fail(error) {
        this.emit('error', {
            error,
            bytesTransferred: this.bytesTransferred,
            totalBytes: this.totalBytes,
        });
    }
}

// ============================================
// DISTRIBUTION MANAGER
// ============================================

/**
 * DistributionManager - Manages model uploads and downloads
 */
export class DistributionManager extends EventEmitter {
    /**
     * Create a new DistributionManager
     * @param {object} options - Configuration options
     */
    constructor(options = {}) {
        super();

        this.id = `dist-${randomBytes(6).toString('hex')}`;

        // GCS configuration
        this.gcsConfig = {
            bucket: options.gcsBucket || DEFAULT_GCS_BUCKET,
            projectId: options.gcsProjectId || process.env.GCS_PROJECT_ID,
            keyFilePath: options.gcsKeyFile || process.env.GOOGLE_APPLICATION_CREDENTIALS,
        };

        // IPFS configuration
        this.ipfsConfig = {
            gateway: options.ipfsGateway || DEFAULT_IPFS_GATEWAY,
            web3StorageToken: options.web3StorageToken || process.env.WEB3_STORAGE_TOKEN,
            nftStorageToken: options.nftStorageToken || process.env.NFT_STORAGE_TOKEN,
        };

        // CDN configuration
        this.cdnConfig = {
            baseUrl: options.cdnBaseUrl || DEFAULT_CDN_BASE,
            fallbackUrls: options.cdnFallbacks || [],
        };

        // Download cache (in-flight downloads)
        this.activeDownloads = new Map();

        // Stats
        this.stats = {
            uploads: 0,
            downloads: 0,
            bytesUploaded: 0,
            bytesDownloaded: 0,
            failures: 0,
        };
    }

    // ============================================
    // URL GENERATION
    // ============================================

    /**
     * Generate CDN URL for a model
     * @param {string} modelName - Model name
     * @param {string} version - Model version
     * @param {string} filename - Filename
     * @returns {string}
     */
    getCdnUrl(modelName, version, filename = null) {
        const file = filename || `${modelName}.onnx`;
        return `${this.cdnConfig.baseUrl}/${modelName}/${version}/${file}`;
    }

    /**
     * Generate GCS URL for a model
     * @param {string} modelName - Model name
     * @param {string} version - Model version
     * @param {string} filename - Filename
     * @returns {string}
     */
    getGcsUrl(modelName, version, filename = null) {
        const file = filename || `${modelName}.onnx`;
        return `gs://${this.gcsConfig.bucket}/${modelName}/${version}/${file}`;
    }

    /**
     * Generate IPFS URL from CID
     * @param {string} cid - IPFS Content ID
     * @returns {string}
     */
    getIpfsUrl(cid) {
        return `ipfs://${cid}`;
    }

    /**
     * Generate HTTP gateway URL for IPFS
     * @param {string} cid - IPFS Content ID
     * @returns {string}
     */
    getIpfsGatewayUrl(cid) {
        // Handle both ipfs:// URLs and raw CIDs
        const cleanCid = cid.replace(/^ipfs:\/\//, '');
        return `${this.ipfsConfig.gateway}/${cleanCid}`;
    }

    /**
     * Generate all source URLs for a model
     * @param {object} sources - Source configuration from metadata
     * @param {string} modelName - Model name
     * @param {string} version - Version
     * @returns {object[]} Sorted list of sources with URLs
     */
    generateSourceUrls(sources, modelName, version) {
        const urls = [];

        // CDN (highest priority)
        if (sources.cdn) {
            urls.push({
                type: 'cdn',
                url: sources.cdn,
                priority: SOURCE_PRIORITY.cdn,
            });
        } else {
            // Auto-generate CDN URL
            urls.push({
                type: 'cdn',
                url: this.getCdnUrl(modelName, version),
                priority: SOURCE_PRIORITY.cdn,
            });
        }

        // GCS
        if (sources.gcs) {
            const gcsMatch = sources.gcs.match(SOURCE_PATTERNS.gcs);
            if (gcsMatch) {
                // Convert gs:// to HTTPS URL
                const [, bucket, path] = gcsMatch;
                urls.push({
                    type: 'gcs',
                    url: `https://storage.googleapis.com/${bucket}/${path}`,
                    originalUrl: sources.gcs,
                    priority: SOURCE_PRIORITY.gcs,
                });
            }
        }

        // IPFS
        if (sources.ipfs) {
            urls.push({
                type: 'ipfs',
                url: this.getIpfsGatewayUrl(sources.ipfs),
                originalUrl: sources.ipfs,
                priority: SOURCE_PRIORITY.ipfs,
            });
        }

        // Fallback URLs
        for (const fallback of this.cdnConfig.fallbackUrls) {
            urls.push({
                type: 'fallback',
                url: `${fallback}/${modelName}/${version}/${modelName}.onnx`,
                priority: SOURCE_PRIORITY.fallback,
            });
        }

        // Sort by priority
        return urls.sort((a, b) => a.priority - b.priority);
    }

    // ============================================
    // DOWNLOAD
    // ============================================

    /**
     * Download a model from the best available source
     * @param {object} metadata - Model metadata
     * @param {object} options - Download options
     * @returns {Promise<Buffer>}
     */
    async download(metadata, options = {}) {
        const { name, version, sources, size, hash } = metadata;
        const key = `${name}@${version}`;

        // Check for in-flight download
        if (this.activeDownloads.has(key)) {
            return this.activeDownloads.get(key);
        }

        const downloadPromise = this._executeDownload(metadata, options);
        this.activeDownloads.set(key, downloadPromise);

        try {
            const result = await downloadPromise;
            return result;
        } finally {
            this.activeDownloads.delete(key);
        }
    }

    /**
     * Execute the download with fallback
     * @private
     */
    async _executeDownload(metadata, options = {}) {
        const { name, version, sources, size, hash } = metadata;
        const sourceUrls = this.generateSourceUrls(sources, name, version);

        const progress = new ProgressTracker(size);

        if (options.onProgress) {
            progress.on('progress', options.onProgress);
        }

        let lastError = null;

        for (const source of sourceUrls) {
            try {
                this.emit('download_attempt', { source, model: name, version });

                const data = await this._downloadFromUrl(source.url, {
                    ...options,
                    progress,
                    expectedSize: size,
                });

                // Verify integrity
                if (hash) {
                    const computedHash = `sha256:${createHash('sha256').update(data).digest('hex')}`;
                    if (computedHash !== hash) {
                        throw new Error(`Hash mismatch: expected ${hash}, got ${computedHash}`);
                    }
                }

                this.stats.downloads++;
                this.stats.bytesDownloaded += data.length;

                progress.complete();

                this.emit('download_complete', {
                    source,
                    model: name,
                    version,
                    size: data.length,
                });

                return data;

            } catch (error) {
                lastError = error;
                this.emit('download_failed', {
                    source,
                    model: name,
                    version,
                    error: error.message,
                });

                // Continue to next source
                continue;
            }
        }

        this.stats.failures++;
        progress.fail(lastError);

        throw new Error(`Failed to download ${name}@${version} from all sources: ${lastError?.message}`);
    }

    /**
     * Download from a URL with streaming and progress
     * @private
     */
    _downloadFromUrl(url, options = {}) {
        return new Promise((resolve, reject) => {
            const { progress, expectedSize, timeout = 60000 } = options;
            const parsedUrl = new URL(url);
            const protocol = parsedUrl.protocol === 'https:' ? https : http;

            const chunks = [];
            let bytesReceived = 0;

            const request = protocol.get(url, {
                timeout,
                headers: {
                    'User-Agent': 'RuVector-EdgeNet/1.0',
                    'Accept': 'application/octet-stream',
                },
            }, (response) => {
                // Handle redirects
                if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
                    this._downloadFromUrl(response.headers.location, options)
                        .then(resolve)
                        .catch(reject);
                    return;
                }

                if (response.statusCode !== 200) {
                    reject(new Error(`HTTP ${response.statusCode}: ${response.statusMessage}`));
                    return;
                }

                const contentLength = parseInt(response.headers['content-length'] || expectedSize || 0, 10);
                if (progress && contentLength) {
                    progress.totalBytes = contentLength;
                }

                response.on('data', (chunk) => {
                    chunks.push(chunk);
                    bytesReceived += chunk.length;

                    if (progress) {
                        progress.update(chunk.length);
                    }
                });

                response.on('end', () => {
                    const data = Buffer.concat(chunks);
                    resolve(data);
                });

                response.on('error', reject);
            });

            request.on('error', reject);
            request.on('timeout', () => {
                request.destroy();
                reject(new Error('Request timeout'));
            });
        });
    }

    /**
     * Download to a file with streaming
     * @param {object} metadata - Model metadata
     * @param {string} destPath - Destination file path
     * @param {object} options - Download options
     */
    async downloadToFile(metadata, destPath, options = {}) {
        const data = await this.download(metadata, options);

        // Ensure directory exists
        const dir = path.dirname(destPath);
        await fs.mkdir(dir, { recursive: true });

        await fs.writeFile(destPath, data);

        return {
            path: destPath,
            size: data.length,
        };
    }

    // ============================================
    // UPLOAD
    // ============================================

    /**
     * Upload a model to Google Cloud Storage
     * @param {Buffer} data - Model data
     * @param {string} modelName - Model name
     * @param {string} version - Version
     * @param {object} options - Upload options
     * @returns {Promise<string>} GCS URL
     */
    async uploadToGcs(data, modelName, version, options = {}) {
        const { filename = `${modelName}.onnx` } = options;
        const gcsPath = `${modelName}/${version}/${filename}`;

        // Check for @google-cloud/storage
        let storage;
        try {
            const { Storage } = await import('@google-cloud/storage');
            storage = new Storage({
                projectId: this.gcsConfig.projectId,
                keyFilename: this.gcsConfig.keyFilePath,
            });
        } catch (error) {
            throw new Error('GCS upload requires @google-cloud/storage package');
        }

        const bucket = storage.bucket(this.gcsConfig.bucket);
        const file = bucket.file(gcsPath);

        const progress = new ProgressTracker(data.length);

        if (options.onProgress) {
            progress.on('progress', options.onProgress);
        }

        await new Promise((resolve, reject) => {
            const stream = file.createWriteStream({
                metadata: {
                    contentType: 'application/octet-stream',
                    metadata: {
                        modelName,
                        version,
                        hash: `sha256:${createHash('sha256').update(data).digest('hex')}`,
                    },
                },
            });

            stream.on('error', reject);
            stream.on('finish', resolve);

            // Write in chunks for progress tracking
            let offset = 0;
            const writeChunk = () => {
                while (offset < data.length) {
                    const end = Math.min(offset + CHUNK_SIZE, data.length);
                    const chunk = data.slice(offset, end);

                    if (!stream.write(chunk)) {
                        offset = end;
                        stream.once('drain', writeChunk);
                        return;
                    }

                    progress.update(chunk.length);
                    offset = end;
                }
                stream.end();
            };

            writeChunk();
        });

        progress.complete();
        this.stats.uploads++;
        this.stats.bytesUploaded += data.length;

        const gcsUrl = this.getGcsUrl(modelName, version, filename);

        this.emit('upload_complete', {
            type: 'gcs',
            url: gcsUrl,
            model: modelName,
            version,
            size: data.length,
        });

        return gcsUrl;
    }

    /**
     * Upload a model to IPFS via web3.storage
     * @param {Buffer} data - Model data
     * @param {string} modelName - Model name
     * @param {string} version - Version
     * @param {object} options - Upload options
     * @returns {Promise<string>} IPFS CID
     */
    async uploadToIpfs(data, modelName, version, options = {}) {
        const { filename = `${modelName}.onnx`, provider = 'web3storage' } = options;

        let cid;

        if (provider === 'web3storage' && this.ipfsConfig.web3StorageToken) {
            cid = await this._uploadToWeb3Storage(data, filename);
        } else if (provider === 'nftstorage' && this.ipfsConfig.nftStorageToken) {
            cid = await this._uploadToNftStorage(data, filename);
        } else {
            throw new Error('No IPFS provider configured. Set WEB3_STORAGE_TOKEN or NFT_STORAGE_TOKEN');
        }

        this.stats.uploads++;
        this.stats.bytesUploaded += data.length;

        const ipfsUrl = this.getIpfsUrl(cid);

        this.emit('upload_complete', {
            type: 'ipfs',
            url: ipfsUrl,
            cid,
            model: modelName,
            version,
            size: data.length,
        });

        return ipfsUrl;
    }

    /**
     * Upload to web3.storage
     * @private
     */
    async _uploadToWeb3Storage(data, filename) {
        // web3.storage API upload
        const response = await this._httpRequest('https://api.web3.storage/upload', {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${this.ipfsConfig.web3StorageToken}`,
                'X-Name': filename,
            },
            body: data,
        });

        if (!response.cid) {
            throw new Error('web3.storage upload failed: no CID returned');
        }

        return response.cid;
    }

    /**
     * Upload to nft.storage
     * @private
     */
    async _uploadToNftStorage(data, filename) {
        // nft.storage API upload
        const response = await this._httpRequest('https://api.nft.storage/upload', {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${this.ipfsConfig.nftStorageToken}`,
            },
            body: data,
        });

        if (!response.value?.cid) {
            throw new Error('nft.storage upload failed: no CID returned');
        }

        return response.value.cid;
    }

    /**
     * Make an HTTP request
     * @private
     */
    _httpRequest(url, options = {}) {
        return new Promise((resolve, reject) => {
            const parsedUrl = new URL(url);
            const protocol = parsedUrl.protocol === 'https:' ? https : http;

            const requestOptions = {
                method: options.method || 'GET',
                headers: options.headers || {},
                hostname: parsedUrl.hostname,
                path: parsedUrl.pathname + parsedUrl.search,
                port: parsedUrl.port,
            };

            const request = protocol.request(requestOptions, (response) => {
                const chunks = [];

                response.on('data', chunk => chunks.push(chunk));
                response.on('end', () => {
                    const body = Buffer.concat(chunks).toString('utf-8');

                    if (response.statusCode >= 400) {
                        reject(new Error(`HTTP ${response.statusCode}: ${body}`));
                        return;
                    }

                    try {
                        resolve(JSON.parse(body));
                    } catch {
                        resolve(body);
                    }
                });
            });

            request.on('error', reject);

            if (options.body) {
                request.write(options.body);
            }

            request.end();
        });
    }

    // ============================================
    // INTEGRITY VERIFICATION
    // ============================================

    /**
     * Compute SHA256 hash of data
     * @param {Buffer} data - Data to hash
     * @returns {string} Hash string with sha256: prefix
     */
    computeHash(data) {
        return `sha256:${createHash('sha256').update(data).digest('hex')}`;
    }

    /**
     * Verify data integrity against expected hash
     * @param {Buffer} data - Data to verify
     * @param {string} expectedHash - Expected hash
     * @returns {boolean}
     */
    verifyIntegrity(data, expectedHash) {
        const computed = this.computeHash(data);
        return computed === expectedHash;
    }

    /**
     * Verify a downloaded model
     * @param {Buffer} data - Model data
     * @param {object} metadata - Model metadata
     * @returns {object} Verification result
     */
    verifyModel(data, metadata) {
        const result = {
            valid: true,
            checks: [],
        };

        // Size check
        if (metadata.size) {
            const sizeMatch = data.length === metadata.size;
            result.checks.push({
                type: 'size',
                expected: metadata.size,
                actual: data.length,
                passed: sizeMatch,
            });
            if (!sizeMatch) result.valid = false;
        }

        // Hash check
        if (metadata.hash) {
            const hashMatch = this.verifyIntegrity(data, metadata.hash);
            result.checks.push({
                type: 'hash',
                expected: metadata.hash,
                actual: this.computeHash(data),
                passed: hashMatch,
            });
            if (!hashMatch) result.valid = false;
        }

        return result;
    }

    // ============================================
    // STATS AND INFO
    // ============================================

    /**
     * Get distribution manager stats
     * @returns {object}
     */
    getStats() {
        return {
            ...this.stats,
            activeDownloads: this.activeDownloads.size,
        };
    }
}

// ============================================
// DEFAULT EXPORT
// ============================================

export default DistributionManager;
