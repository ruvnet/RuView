import axios from 'axios';

jest.mock('axios', () => {
  const mockAxiosInstance = {
    request: jest.fn(),
  };
  const mockAxios = {
    create: jest.fn(() => mockAxiosInstance),
    isAxiosError: jest.fn(),
    __mockInstance: mockAxiosInstance,
  };
  return {
    __esModule: true,
    default: mockAxios,
    ...mockAxios,
  };
});

// Import after mocking so the mock takes effect
const { apiService } = require('@/services/api.service');
const mockAxios = axios as jest.Mocked<typeof axios> & { __mockInstance: { request: jest.Mock } };

describe('ApiService', () => {
  const mockRequest = mockAxios.__mockInstance.request;

  beforeEach(() => {
    jest.clearAllMocks();
    apiService.setBaseUrl('');
  });

  describe('setBaseUrl', () => {
    it('stores the base URL', () => {
      apiService.setBaseUrl('http://10.0.0.1:3000');
      mockRequest.mockResolvedValueOnce({ data: { ok: true } });
      apiService.get('/test');
      expect(mockRequest).toHaveBeenCalledWith(
        expect.objectContaining({ url: 'http://10.0.0.1:3000/test' }),
      );
    });

    it('handles null by falling back to empty string', () => {
      apiService.setBaseUrl(null as unknown as string);
      mockRequest.mockResolvedValueOnce({ data: {} });
      apiService.get('/api/status');
      expect(mockRequest).toHaveBeenCalledWith(
        expect.objectContaining({ url: '/api/status' }),
      );
    });
  });

  describe('buildUrl (via get)', () => {
    it('concatenates baseUrl and path', () => {
      apiService.setBaseUrl('http://example.com');
      mockRequest.mockResolvedValueOnce({ data: {} });
      apiService.get('/api/v1/status');
      expect(mockRequest).toHaveBeenCalledWith(
        expect.objectContaining({ url: 'http://example.com/api/v1/status' }),
      );
    });

    it('removes trailing slash from baseUrl', () => {
      apiService.setBaseUrl('http://example.com/');
      mockRequest.mockResolvedValueOnce({ data: {} });
      apiService.get('/test');
      expect(mockRequest).toHaveBeenCalledWith(
        expect.objectContaining({ url: 'http://example.com/test' }),
      );
    });

    it('uses path as-is when baseUrl is empty', () => {
      apiService.setBaseUrl('');
      mockRequest.mockResolvedValueOnce({ data: {} });
      apiService.get('/standalone');
      expect(mockRequest).toHaveBeenCalledWith(
        expect.objectContaining({ url: '/standalone' }),
      );
    });

    it('uses the full URL path if path starts with http', () => {
      apiService.setBaseUrl('http://base.com');
      mockRequest.mockResolvedValueOnce({ data: {} });
      apiService.get('https://other.com/endpoint');
      expect(mockRequest).toHaveBeenCalledWith(
        expect.objectContaining({ url: 'https://other.com/endpoint' }),
      );
    });
  });

  describe('get', () => {
    it('returns response data on success', async () => {
      apiService.setBaseUrl('http://localhost:3000');
      mockRequest.mockResolvedValueOnce({ data: { status: 'ok' } });
      const result = await apiService.get('/api/v1/pose/status');
      expect(result).toEqual({ status: 'ok' });
    });

    it('uses GET method', () => {
      mockRequest.mockResolvedValueOnce({ data: {} });
      apiService.get('/test');
      expect(mockRequest).toHaveBeenCalledWith(
        expect.objectContaining({ method: 'GET' }),
      );
    });
  });

  describe('post', () => {
    it('sends body data', () => {
      apiService.setBaseUrl('http://localhost:3000');
      mockRequest.mockResolvedValueOnce({ data: { id: 1 } });
      apiService.post('/api/events', { name: 'test' });
      expect(mockRequest).toHaveBeenCalledWith(
        expect.objectContaining({
          method: 'POST',
          data: { name: 'test' },
        }),
      );
    });
  });

  describe('error normalization', () => {
    it('normalizes axios error with response data message', async () => {
      const axiosError = {
        message: 'Request failed with status code 400',
        response: {
          status: 400,
          data: { message: 'Bad request body' },
        },
        code: 'ERR_BAD_REQUEST',
        isAxiosError: true,
      };
      mockRequest.mockRejectedValue(axiosError);
      (mockAxios.isAxiosError as jest.Mock).mockReturnValue(true);

      await expect(apiService.get('/test')).rejects.toEqual(
        expect.objectContaining({
          message: 'Bad request body',
          status: 400,
          code: 'ERR_BAD_REQUEST',
        }),
      );
    });

    it('normalizes generic Error', async () => {
      mockRequest.mockRejectedValue(new Error('network timeout'));
      (mockAxios.isAxiosError as jest.Mock).mockReturnValue(false);

      await expect(apiService.get('/test')).rejects.toEqual(
        expect.objectContaining({ message: 'network timeout' }),
      );
    });

    it('normalizes unknown error', async () => {
      mockRequest.mockRejectedValue('string error');
      (mockAxios.isAxiosError as jest.Mock).mockReturnValue(false);

      await expect(apiService.get('/test')).rejects.toEqual(
        expect.objectContaining({ message: 'Unknown error' }),
      );
    });
  });

  describe('retry logic', () => {
    it('retries up to 2 times on failure then throws', async () => {
      const error = new Error('fail');
      mockRequest.mockRejectedValue(error);
      (mockAxios.isAxiosError as jest.Mock).mockReturnValue(false);

      await expect(apiService.get('/flaky')).rejects.toEqual(
        expect.objectContaining({ message: 'fail' }),
      );
      // 1 initial + 2 retries = 3 total calls
      expect(mockRequest).toHaveBeenCalledTimes(3);
    });

    it('succeeds on second attempt without throwing', async () => {
      mockRequest
        .mockRejectedValueOnce(new Error('transient'))
        .mockResolvedValueOnce({ data: { recovered: true } });

      const result = await apiService.get('/flaky');
      expect(result).toEqual({ recovered: true });
      expect(mockRequest).toHaveBeenCalledTimes(2);
    });
  });
});
