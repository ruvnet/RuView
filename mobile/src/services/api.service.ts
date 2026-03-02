import axios, { type AxiosError, type AxiosInstance, type AxiosRequestConfig } from 'axios';
import { API_POSE_FRAMES_PATH, API_POSE_STATUS_PATH, API_POSE_ZONES_PATH } from '@/constants/api';
import type { ApiError, HistoricalFrames, PoseStatus, ZoneConfig } from '@/types/api';

class ApiService {
  private baseUrl = '';
  private client: AxiosInstance;

  constructor() {
    this.client = axios.create({
      timeout: 5000,
      headers: {
        Accept: 'application/json',
        'Content-Type': 'application/json',
      },
    });
  }

  setBaseUrl(url: string): void {
    this.baseUrl = url ?? '';
  }

  private buildUrl(path: string): string {
    if (!this.baseUrl) {
      return path;
    }
    if (path.startsWith('http://') || path.startsWith('https://')) {
      return path;
    }
    const normalized = this.baseUrl.replace(/\/$/, '');
    return `${normalized}${path.startsWith('/') ? path : `/${path}`}`;
  }

  private normalizeError(error: unknown): ApiError {
    if (axios.isAxiosError(error)) {
      const axiosError = error as AxiosError<{ message?: string }>;
      const message =
        axiosError.response?.data && typeof axiosError.response.data === 'object' && 'message' in axiosError.response.data
          ? String((axiosError.response.data as { message?: string }).message)
          : axiosError.message || 'Request failed';
      return {
        message,
        status: axiosError.response?.status,
        code: axiosError.code,
        details: axiosError.response?.data,
      };
    }

    if (error instanceof Error) {
      return { message: error.message };
    }

    return { message: 'Unknown error' };
  }

  private async requestWithRetry<T>(config: AxiosRequestConfig, retriesLeft: number): Promise<T> {
    try {
      const response = await this.client.request<T>({
        ...config,
        url: this.buildUrl(config.url || ''),
      });
      return response.data;
    } catch (error) {
      if (retriesLeft > 0) {
        return this.requestWithRetry<T>(config, retriesLeft - 1);
      }
      throw this.normalizeError(error);
    }
  }

  get<T>(path: string): Promise<T> {
    return this.requestWithRetry<T>({ method: 'GET', url: path }, 2);
  }

  post<T>(path: string, body: unknown): Promise<T> {
    return this.requestWithRetry<T>({ method: 'POST', url: path, data: body }, 2);
  }

  getStatus(): Promise<PoseStatus> {
    return this.get<PoseStatus>(API_POSE_STATUS_PATH);
  }

  getZones(): Promise<ZoneConfig[]> {
    return this.get<ZoneConfig[]>(API_POSE_ZONES_PATH);
  }

  getFrames(limit: number): Promise<HistoricalFrames> {
    return this.get<HistoricalFrames>(`${API_POSE_FRAMES_PATH}?limit=${encodeURIComponent(String(limit))}`);
  }
}

export const apiService = new ApiService();
