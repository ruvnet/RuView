// API Service for WiFi-DensePose UI

import { API_CONFIG, buildApiUrl } from '../config/api.config.js';
import { backendDetector } from '../utils/backend-detector.js';

export class ApiService {
  constructor() {
    this.authToken = null;
    this.requestInterceptors = [];
    this.responseInterceptors = [];
  }

  // Set authentication token
  setAuthToken(token) {
    this.authToken = token;
  }

  // Add request interceptor
  addRequestInterceptor(interceptor) {
    this.requestInterceptors.push(interceptor);
  }

  // Add response interceptor
  addResponseInterceptor(interceptor) {
    this.responseInterceptors.push(interceptor);
  }

  // Build headers for requests
  getHeaders(customHeaders = {}) {
    const headers = {
      ...API_CONFIG.DEFAULT_HEADERS,
      ...customHeaders
    };

    if (this.authToken) {
      headers['Authorization'] = `Bearer ${this.authToken}`;
    }

    return headers;
  }

  // Process request through interceptors
  async processRequest(url, options) {
    let processedUrl = url;
    let processedOptions = options;

    for (const interceptor of this.requestInterceptors) {
      const result = await interceptor(processedUrl, processedOptions);
      processedUrl = result.url || processedUrl;
      processedOptions = result.options || processedOptions;
    }

    return { url: processedUrl, options: processedOptions };
  }

  // Process response through interceptors
  async processResponse(response, url) {
    let processedResponse = response;

    for (const interceptor of this.responseInterceptors) {
      processedResponse = await interceptor(processedResponse, url);
    }

    return processedResponse;
  }

  // Generic request method
  async request(url, options = {}) {
    try {
      // Process request through interceptors
      const processed = await this.processRequest(url, options);
      
      // Determine the correct base URL (real backend vs mock)
      let finalUrl = processed.url;
      if (processed.url.startsWith(API_CONFIG.BASE_URL)) {
        const baseUrl = await backendDetector.getBaseUrl();
        finalUrl = processed.url.replace(API_CONFIG.BASE_URL, baseUrl);
      }
      
      // Make the request
      const response = await fetch(finalUrl, {
        ...processed.options,
        headers: this.getHeaders(processed.options.headers)
      });

      // Process response through interceptors
      const processedResponse = await this.processResponse(response, url);

      // Handle errors
      if (!processedResponse.ok) {
        const error = await processedResponse.json().catch(() => ({
          message: `HTTP ${processedResponse.status}: ${processedResponse.statusText}`
        }));
        throw new Error(error.message || error.detail || 'Request failed');
      }

      // Parse JSON response
      const data = await processedResponse.json().catch(() => null);
      return data;

    } catch (error) {
      console.error('API Request Error:', error);
      throw error;
    }
  }

  // GET request
  async get(endpoint, params = {}, options = {}) {
    const url = buildApiUrl(endpoint, params);
    return this.request(url, {
      method: 'GET',
      ...options
    });
  }

  // POST request
  async post(endpoint, data = {}, options = {}) {
    const url = buildApiUrl(endpoint);
    return this.request(url, {
      method: 'POST',
      body: JSON.stringify(data),
      ...options
    });
  }

  // PUT request
  async put(endpoint, data = {}, options = {}) {
    const url = buildApiUrl(endpoint);
    return this.request(url, {
      method: 'PUT',
      body: JSON.stringify(data),
      ...options
    });
  }

  // DELETE request
  async delete(endpoint, options = {}) {
    const url = buildApiUrl(endpoint);
    return this.request(url, {
      method: 'DELETE',
      ...options
    });
  }
}

// Create singleton instance
export const apiService = new ApiService();