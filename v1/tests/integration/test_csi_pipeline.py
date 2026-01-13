import pytest
import torch
import numpy as np
from unittest.mock import Mock, patch, MagicMock
from src.core.csi_processor import CSIProcessor
from src.core.phase_sanitizer import PhaseSanitizer
from src.hardware.router_interface import RouterInterface
from src.hardware.csi_extractor import CSIExtractor


class TestCSIPipeline:
    """Integration tests for CSI processing pipeline following London School TDD principles"""
    
    @pytest.fixture
    def mock_router_config(self):
        """Configuration for router interface"""
        return {
            'router_ip': '192.168.1.1',
            'username': 'admin',
            'password': 'password',
            'ssh_port': 22,
            'timeout': 30,
            'max_retries': 3
        }
    
    @pytest.fixture
    def mock_extractor_config(self):
        """Configuration for CSI extractor"""
        return {
            'interface': 'wlan0',
            'channel': 6,
            'bandwidth': 20,
            'antenna_count': 3,
            'subcarrier_count': 56,
            'sample_rate': 1000,
            'buffer_size': 1024
        }
    
    @pytest.fixture
    def mock_processor_config(self):
        """Configuration for CSI processor"""
        return {
            'window_size': 100,
            'overlap': 0.5,
            'filter_type': 'butterworth',
            'filter_order': 4,
            'cutoff_frequency': 50,
            'normalization': 'minmax',
            'outlier_threshold': 3.0
        }
    
    @pytest.fixture
    def mock_sanitizer_config(self):
        """Configuration for phase sanitizer"""
        return {
            'unwrap_method': 'numpy',
            'smoothing_window': 5,
            'outlier_threshold': 2.0,
            'interpolation_method': 'linear',
            'phase_correction': True
        }
    
    @pytest.fixture
    def csi_pipeline_components(self, mock_router_config, mock_extractor_config, 
                               mock_processor_config, mock_sanitizer_config):
        """Create CSI pipeline components for testing"""
        router = RouterInterface(mock_router_config)
        extractor = CSIExtractor(mock_extractor_config)
        processor = CSIProcessor(mock_processor_config)
        sanitizer = PhaseSanitizer(mock_sanitizer_config)
        
        return {
            'router': router,
            'extractor': extractor,
            'processor': processor,
            'sanitizer': sanitizer
        }
    
    @pytest.fixture
    def mock_raw_csi_data(self):
        """Generate mock raw CSI data"""
        batch_size = 10
        antennas = 3
        subcarriers = 56
        time_samples = 100
        
        # Generate complex CSI data
        real_part = np.random.randn(batch_size, antennas, subcarriers, time_samples)
        imag_part = np.random.randn(batch_size, antennas, subcarriers, time_samples)
        
        return {
            'csi_data': real_part + 1j * imag_part,
            'timestamps': np.linspace(0, 1, time_samples),
            'metadata': {
                'channel': 6,
                'bandwidth': 20,
                'rssi': -45,
                'noise_floor': -90
            }
        }
    
    def test_end_to_end_csi_pipeline_processes_data_correctly(self, csi_pipeline_components, mock_raw_csi_data):
        """Test that end-to-end CSI pipeline processes data correctly"""
        # Arrange
        router = csi_pipeline_components['router']
        extractor = csi_pipeline_components['extractor']
        processor = csi_pipeline_components['processor']
        sanitizer = csi_pipeline_components['sanitizer']
        
        # Mock the hardware extraction
        with patch.object(extractor, 'extract_csi_data', return_value=mock_raw_csi_data):
            with patch.object(router, 'connect', return_value=True):
                with patch.object(router, 'configure_monitor_mode', return_value=True):
                    
                    # Act - Run the pipeline
                    # 1. Connect to router and configure
                    router.connect()
                    router.configure_monitor_mode('wlan0', 6)
                    
                    # 2. Extract CSI data
                    raw_data = extractor.extract_csi_data()
                    
                    # 3. Process CSI data
                    processed_data = processor.process_csi_batch(raw_data['csi_data'])
                    
                    # 4. Sanitize phase information
                    sanitized_data = sanitizer.sanitize_phase_batch(processed_data)
                    
                    # Assert
                    assert raw_data is not None
                    assert processed_data is not None
                    assert sanitized_data is not None
                    
                    # Check data flow integrity
                    assert isinstance(processed_data, torch.Tensor)
                    assert isinstance(sanitized_data, torch.Tensor)
                    assert processed_data.shape == sanitized_data.shape
    
    def test_pipeline_handles_hardware_connection_failure(self, csi_pipeline_components):
        """Test that pipeline handles hardware connection failures gracefully"""
        # Arrange
        router = csi_pipeline_components['router']
        
        # Mock connection failure
        with patch.object(router, 'connect', return_value=False):
            
            # Act & Assert
            connection_result = router.connect()
            assert connection_result is False
            
            # Pipeline should handle this gracefully
            with pytest.raises(Exception):  # Should raise appropriate exception
                router.configure_monitor_mode('wlan0', 6)
    
    def test_pipeline_handles_csi_extraction_timeout(self, csi_pipeline_components):
        """Test that pipeline handles CSI extraction timeouts"""
        # Arrange
        extractor = csi_pipeline_components['extractor']
        
        # Mock extraction timeout
        with patch.object(extractor, 'extract_csi_data', side_effect=TimeoutError("CSI extraction timeout")):
            
            # Act & Assert
            with pytest.raises(TimeoutError):
                extractor.extract_csi_data()
    
    def test_pipeline_handles_invalid_csi_data_format(self, csi_pipeline_components):
        """Test that pipeline handles invalid CSI data formats"""
        # Arrange
        processor = csi_pipeline_components['processor']
        
        # Invalid data format
        invalid_data = np.random.randn(10, 2, 56)  # Missing time dimension
        
        # Act & Assert
        with pytest.raises(ValueError):
            processor.process_csi_batch(invalid_data)
    
    def test_pipeline_maintains_data_consistency_across_stages(self, csi_pipeline_components, mock_raw_csi_data):
        """Test that pipeline maintains data consistency across processing stages"""
        # Arrange
        processor = csi_pipeline_components['processor']
        sanitizer = csi_pipeline_components['sanitizer']
        
        csi_data = mock_raw_csi_data['csi_data']
        
        # Act
        processed_data = processor.process_csi_batch(csi_data)
        sanitized_data = sanitizer.sanitize_phase_batch(processed_data)
        
        # Assert - Check data consistency
        assert processed_data.shape[0] == sanitized_data.shape[0]  # Batch size preserved
        assert processed_data.shape[1] == sanitized_data.shape[1]  # Antenna count preserved
        assert processed_data.shape[2] == sanitized_data.shape[2]  # Subcarrier count preserved
        
        # Check that data is not corrupted (no NaN or infinite values)
        assert not torch.isnan(processed_data).any()
        assert not torch.isinf(processed_data).any()
        assert not torch.isnan(sanitized_data).any()
        assert not torch.isinf(sanitized_data).any()
    
    def test_pipeline_performance_meets_real_time_requirements(self, csi_pipeline_components, mock_raw_csi_data):
        """Test that pipeline performance meets real-time processing requirements"""
        import time
        
        # Arrange
        processor = csi_pipeline_components['processor']
        sanitizer = csi_pipeline_components['sanitizer']
        
        csi_data = mock_raw_csi_data['csi_data']
        
        # Act - Measure processing time
        start_time = time.time()
        
        processed_data = processor.process_csi_batch(csi_data)
        sanitized_data = sanitizer.sanitize_phase_batch(processed_data)
        
        end_time = time.time()
        processing_time = end_time - start_time
        
        # Assert - Should process within reasonable time (< 100ms for this data size)
        assert processing_time < 0.1, f"Processing took {processing_time:.3f}s, expected < 0.1s"
    
    def test_pipeline_handles_different_data_sizes(self, csi_pipeline_components):
        """Test that pipeline handles different CSI data sizes"""
        # Arrange
        processor = csi_pipeline_components['processor']
        sanitizer = csi_pipeline_components['sanitizer']
        
        # Different data sizes
        small_data = np.random.randn(1, 3, 56, 50) + 1j * np.random.randn(1, 3, 56, 50)
        large_data = np.random.randn(20, 3, 56, 200) + 1j * np.random.randn(20, 3, 56, 200)
        
        # Act
        small_processed = processor.process_csi_batch(small_data)
        small_sanitized = sanitizer.sanitize_phase_batch(small_processed)
        
        large_processed = processor.process_csi_batch(large_data)
        large_sanitized = sanitizer.sanitize_phase_batch(large_processed)
        
        # Assert
        assert small_processed.shape == small_sanitized.shape
        assert large_processed.shape == large_sanitized.shape
        assert small_processed.shape != large_processed.shape  # Different sizes
    
    def test_pipeline_configuration_validation(self, mock_router_config, mock_extractor_config, 
                                             mock_processor_config, mock_sanitizer_config):
        """Test that pipeline components validate configurations properly"""
        # Arrange - Invalid configurations
        invalid_router_config = mock_router_config.copy()
        invalid_router_config['router_ip'] = 'invalid_ip'
        
        invalid_extractor_config = mock_extractor_config.copy()
        invalid_extractor_config['antenna_count'] = 0
        
        invalid_processor_config = mock_processor_config.copy()
        invalid_processor_config['window_size'] = -1
        
        invalid_sanitizer_config = mock_sanitizer_config.copy()
        invalid_sanitizer_config['smoothing_window'] = 0
        
        # Act & Assert
        with pytest.raises(ValueError):
            RouterInterface(invalid_router_config)
        
        with pytest.raises(ValueError):
            CSIExtractor(invalid_extractor_config)
        
        with pytest.raises(ValueError):
            CSIProcessor(invalid_processor_config)
        
        with pytest.raises(ValueError):
            PhaseSanitizer(invalid_sanitizer_config)
    
    def test_pipeline_error_recovery_and_logging(self, csi_pipeline_components, mock_raw_csi_data):
        """Test that pipeline handles errors gracefully and logs appropriately"""
        # Arrange
        processor = csi_pipeline_components['processor']
        
        # Corrupt some data to trigger error handling
        corrupted_data = mock_raw_csi_data['csi_data'].copy()
        corrupted_data[0, 0, 0, :] = np.inf  # Introduce infinite values
        
        # Act & Assert
        with pytest.raises(ValueError):  # Should detect and handle corrupted data
            processor.process_csi_batch(corrupted_data)
    
    def test_pipeline_memory_usage_optimization(self, csi_pipeline_components):
        """Test that pipeline optimizes memory usage for large datasets"""
        # Arrange
        processor = csi_pipeline_components['processor']
        sanitizer = csi_pipeline_components['sanitizer']
        
        # Large dataset
        large_data = np.random.randn(100, 3, 56, 1000) + 1j * np.random.randn(100, 3, 56, 1000)
        
        # Act - Process in chunks to test memory optimization
        chunk_size = 10
        results = []
        
        for i in range(0, large_data.shape[0], chunk_size):
            chunk = large_data[i:i+chunk_size]
            processed_chunk = processor.process_csi_batch(chunk)
            sanitized_chunk = sanitizer.sanitize_phase_batch(processed_chunk)
            results.append(sanitized_chunk)
        
        # Assert
        assert len(results) == 10  # 100 samples / 10 chunk_size
        for result in results:
            assert result.shape[0] <= chunk_size
    
    def test_pipeline_supports_concurrent_processing(self, csi_pipeline_components, mock_raw_csi_data):
        """Test that pipeline supports concurrent processing of multiple streams"""
        import threading
        import queue
        
        # Arrange
        processor = csi_pipeline_components['processor']
        sanitizer = csi_pipeline_components['sanitizer']
        
        results_queue = queue.Queue()
        
        def process_stream(stream_id, data):
            try:
                processed = processor.process_csi_batch(data)
                sanitized = sanitizer.sanitize_phase_batch(processed)
                results_queue.put((stream_id, sanitized))
            except Exception as e:
                results_queue.put((stream_id, e))
        
        # Act - Process multiple streams concurrently
        threads = []
        for i in range(3):
            thread = threading.Thread(
                target=process_stream, 
                args=(i, mock_raw_csi_data['csi_data'])
            )
            threads.append(thread)
            thread.start()
        
        # Wait for all threads to complete
        for thread in threads:
            thread.join()
        
        # Assert
        results = []
        while not results_queue.empty():
            results.append(results_queue.get())
        
        assert len(results) == 3
        for stream_id, result in results:
            assert isinstance(result, torch.Tensor)
            assert not isinstance(result, Exception)