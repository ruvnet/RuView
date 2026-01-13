"""TDD tests for router interface following London School approach."""

import pytest
import asyncio
import sys
import os
from unittest.mock import Mock, patch, AsyncMock, MagicMock
from datetime import datetime, timezone
import importlib.util

# Import the router interface module directly
import unittest.mock

# Mock asyncssh before importing
with unittest.mock.patch.dict('sys.modules', {'asyncssh': unittest.mock.MagicMock()}):
    spec = importlib.util.spec_from_file_location(
        'router_interface', 
        '/workspaces/wifi-densepose/src/hardware/router_interface.py'
    )
    router_module = importlib.util.module_from_spec(spec)

    # Import CSI extractor for dependency
    csi_spec = importlib.util.spec_from_file_location(
        'csi_extractor', 
        '/workspaces/wifi-densepose/src/hardware/csi_extractor.py'
    )
    csi_module = importlib.util.module_from_spec(csi_spec)
    csi_spec.loader.exec_module(csi_module)

    # Now load the router interface
    router_module.CSIData = csi_module.CSIData  # Make CSIData available
    spec.loader.exec_module(router_module)

# Get classes from modules
RouterInterface = router_module.RouterInterface
RouterConnectionError = router_module.RouterConnectionError
CSIData = csi_module.CSIData


@pytest.mark.unit
@pytest.mark.tdd
@pytest.mark.london
class TestRouterInterface:
    """Test router interface using London School TDD."""

    @pytest.fixture
    def mock_logger(self):
        """Mock logger for testing."""
        return Mock()

    @pytest.fixture
    def router_config(self):
        """Router configuration for testing."""
        return {
            'host': '192.168.1.1',
            'port': 22,
            'username': 'admin',
            'password': 'password',
            'command_timeout': 30,
            'connection_timeout': 10,
            'max_retries': 3,
            'retry_delay': 1.0
        }

    @pytest.fixture
    def router_interface(self, router_config, mock_logger):
        """Create router interface for testing."""
        return RouterInterface(config=router_config, logger=mock_logger)

    # Initialization tests
    def test_should_initialize_with_valid_config(self, router_config, mock_logger):
        """Should initialize router interface with valid configuration."""
        interface = RouterInterface(config=router_config, logger=mock_logger)
        
        assert interface.host == '192.168.1.1'
        assert interface.port == 22
        assert interface.username == 'admin'
        assert interface.password == 'password'
        assert interface.command_timeout == 30
        assert interface.connection_timeout == 10
        assert interface.max_retries == 3
        assert interface.retry_delay == 1.0
        assert interface.is_connected == False
        assert interface.logger == mock_logger

    def test_should_raise_error_with_invalid_config(self, mock_logger):
        """Should raise error when initialized with invalid configuration."""
        invalid_config = {'invalid': 'config'}
        
        with pytest.raises(ValueError, match="Missing required configuration"):
            RouterInterface(config=invalid_config, logger=mock_logger)

    def test_should_validate_required_fields(self, mock_logger):
        """Should validate all required configuration fields."""
        required_fields = ['host', 'port', 'username', 'password']
        base_config = {
            'host': '192.168.1.1',
            'port': 22,
            'username': 'admin',
            'password': 'password'
        }
        
        for field in required_fields:
            config = base_config.copy()
            del config[field]
            
            with pytest.raises(ValueError, match="Missing required configuration"):
                RouterInterface(config=config, logger=mock_logger)

    def test_should_use_default_values(self, mock_logger):
        """Should use default values for optional parameters."""
        minimal_config = {
            'host': '192.168.1.1',
            'port': 22,
            'username': 'admin',
            'password': 'password'
        }
        
        interface = RouterInterface(config=minimal_config, logger=mock_logger)
        
        assert interface.command_timeout == 30  # default
        assert interface.connection_timeout == 10  # default
        assert interface.max_retries == 3  # default
        assert interface.retry_delay == 1.0  # default

    def test_should_initialize_without_logger(self, router_config):
        """Should initialize without logger provided."""
        interface = RouterInterface(config=router_config)
        
        assert interface.logger is not None  # Should create default logger

    # Connection tests
    @pytest.mark.asyncio
    async def test_should_connect_successfully(self, router_interface):
        """Should establish SSH connection successfully."""
        mock_ssh_client = Mock()
        
        with patch('src.hardware.router_interface.asyncssh.connect', new_callable=AsyncMock) as mock_connect:
            mock_connect.return_value = mock_ssh_client
            
            result = await router_interface.connect()
            
            assert result == True
            assert router_interface.is_connected == True
            assert router_interface.ssh_client == mock_ssh_client
            mock_connect.assert_called_once_with(
                '192.168.1.1',
                port=22,
                username='admin',
                password='password',
                connect_timeout=10
            )

    @pytest.mark.asyncio
    async def test_should_handle_connection_failure(self, router_interface):
        """Should handle SSH connection failure gracefully."""
        with patch('src.hardware.router_interface.asyncssh.connect', new_callable=AsyncMock) as mock_connect:
            mock_connect.side_effect = ConnectionError("Connection failed")
            
            result = await router_interface.connect()
            
            assert result == False
            assert router_interface.is_connected == False
            assert router_interface.ssh_client is None
            router_interface.logger.error.assert_called()

    @pytest.mark.asyncio
    async def test_should_disconnect_when_connected(self, router_interface):
        """Should disconnect SSH connection when connected."""
        mock_ssh_client = Mock()
        router_interface.is_connected = True
        router_interface.ssh_client = mock_ssh_client
        
        await router_interface.disconnect()
        
        assert router_interface.is_connected == False
        assert router_interface.ssh_client is None
        mock_ssh_client.close.assert_called_once()

    @pytest.mark.asyncio
    async def test_should_handle_disconnect_when_not_connected(self, router_interface):
        """Should handle disconnect when not connected."""
        router_interface.is_connected = False
        router_interface.ssh_client = None
        
        await router_interface.disconnect()
        
        # Should not raise any exception
        assert router_interface.is_connected == False

    # Command execution tests
    @pytest.mark.asyncio
    async def test_should_execute_command_successfully(self, router_interface):
        """Should execute SSH command successfully."""
        mock_ssh_client = Mock()
        mock_result = Mock()
        mock_result.stdout = "command output"
        mock_result.stderr = ""
        mock_result.returncode = 0
        
        router_interface.is_connected = True
        router_interface.ssh_client = mock_ssh_client
        
        with patch.object(mock_ssh_client, 'run', new_callable=AsyncMock) as mock_run:
            mock_run.return_value = mock_result
            
            result = await router_interface.execute_command("test command")
            
            assert result == "command output"
            mock_run.assert_called_once_with("test command", timeout=30)

    @pytest.mark.asyncio
    async def test_should_handle_command_execution_when_not_connected(self, router_interface):
        """Should handle command execution when not connected."""
        router_interface.is_connected = False
        
        with pytest.raises(RouterConnectionError, match="Not connected to router"):
            await router_interface.execute_command("test command")

    @pytest.mark.asyncio
    async def test_should_handle_command_execution_error(self, router_interface):
        """Should handle command execution errors."""
        mock_ssh_client = Mock()
        mock_result = Mock()
        mock_result.stdout = ""
        mock_result.stderr = "command error"
        mock_result.returncode = 1
        
        router_interface.is_connected = True
        router_interface.ssh_client = mock_ssh_client
        
        with patch.object(mock_ssh_client, 'run', new_callable=AsyncMock) as mock_run:
            mock_run.return_value = mock_result
            
            with pytest.raises(RouterConnectionError, match="Command failed"):
                await router_interface.execute_command("test command")

    @pytest.mark.asyncio
    async def test_should_retry_command_execution_on_failure(self, router_interface):
        """Should retry command execution on temporary failure."""
        mock_ssh_client = Mock()
        mock_success_result = Mock()
        mock_success_result.stdout = "success output"
        mock_success_result.stderr = ""
        mock_success_result.returncode = 0
        
        router_interface.is_connected = True
        router_interface.ssh_client = mock_ssh_client
        
        with patch.object(mock_ssh_client, 'run', new_callable=AsyncMock) as mock_run:
            # First two calls fail, third succeeds
            mock_run.side_effect = [
                ConnectionError("Network error"),
                ConnectionError("Network error"),
                mock_success_result
            ]
            
            result = await router_interface.execute_command("test command")
            
            assert result == "success output"
            assert mock_run.call_count == 3

    @pytest.mark.asyncio
    async def test_should_fail_after_max_retries(self, router_interface):
        """Should fail after maximum retries exceeded."""
        mock_ssh_client = Mock()
        
        router_interface.is_connected = True
        router_interface.ssh_client = mock_ssh_client
        
        with patch.object(mock_ssh_client, 'run', new_callable=AsyncMock) as mock_run:
            mock_run.side_effect = ConnectionError("Network error")
            
            with pytest.raises(RouterConnectionError, match="Command execution failed after 3 retries"):
                await router_interface.execute_command("test command")
            
            assert mock_run.call_count == 3

    # CSI data retrieval tests
    @pytest.mark.asyncio
    async def test_should_get_csi_data_successfully(self, router_interface):
        """Should retrieve CSI data successfully."""
        expected_csi_data = Mock(spec=CSIData)
        
        with patch.object(router_interface, 'execute_command', new_callable=AsyncMock) as mock_execute:
            with patch.object(router_interface, '_parse_csi_response', return_value=expected_csi_data) as mock_parse:
                mock_execute.return_value = "csi data response"
                
                result = await router_interface.get_csi_data()
                
                assert result == expected_csi_data
                mock_execute.assert_called_once_with("iwlist scan | grep CSI")
                mock_parse.assert_called_once_with("csi data response")

    @pytest.mark.asyncio
    async def test_should_handle_csi_data_retrieval_failure(self, router_interface):
        """Should handle CSI data retrieval failure."""
        with patch.object(router_interface, 'execute_command', new_callable=AsyncMock) as mock_execute:
            mock_execute.side_effect = RouterConnectionError("Command failed")
            
            with pytest.raises(RouterConnectionError):
                await router_interface.get_csi_data()

    # Router status tests
    @pytest.mark.asyncio
    async def test_should_get_router_status_successfully(self, router_interface):
        """Should get router status successfully."""
        expected_status = {
            'cpu_usage': 25.5,
            'memory_usage': 60.2,
            'wifi_status': 'active',
            'uptime': '5 days, 3 hours'
        }
        
        with patch.object(router_interface, 'execute_command', new_callable=AsyncMock) as mock_execute:
            with patch.object(router_interface, '_parse_status_response', return_value=expected_status) as mock_parse:
                mock_execute.return_value = "status response"
                
                result = await router_interface.get_router_status()
                
                assert result == expected_status
                mock_execute.assert_called_once_with("cat /proc/stat && free && iwconfig")
                mock_parse.assert_called_once_with("status response")

    # Configuration tests
    @pytest.mark.asyncio
    async def test_should_configure_csi_monitoring_successfully(self, router_interface):
        """Should configure CSI monitoring successfully."""
        config = {
            'channel': 6,
            'bandwidth': 20,
            'sample_rate': 100
        }
        
        with patch.object(router_interface, 'execute_command', new_callable=AsyncMock) as mock_execute:
            mock_execute.return_value = "Configuration applied"
            
            result = await router_interface.configure_csi_monitoring(config)
            
            assert result == True
            mock_execute.assert_called_once_with(
                "iwconfig wlan0 channel 6 && echo 'CSI monitoring configured'"
            )

    @pytest.mark.asyncio
    async def test_should_handle_csi_monitoring_configuration_failure(self, router_interface):
        """Should handle CSI monitoring configuration failure."""
        config = {
            'channel': 6,
            'bandwidth': 20,
            'sample_rate': 100
        }
        
        with patch.object(router_interface, 'execute_command', new_callable=AsyncMock) as mock_execute:
            mock_execute.side_effect = RouterConnectionError("Command failed")
            
            result = await router_interface.configure_csi_monitoring(config)
            
            assert result == False

    # Health check tests
    @pytest.mark.asyncio
    async def test_should_perform_health_check_successfully(self, router_interface):
        """Should perform health check successfully."""
        with patch.object(router_interface, 'execute_command', new_callable=AsyncMock) as mock_execute:
            mock_execute.return_value = "pong"
            
            result = await router_interface.health_check()
            
            assert result == True
            mock_execute.assert_called_once_with("echo 'ping' && echo 'pong'")

    @pytest.mark.asyncio
    async def test_should_handle_health_check_failure(self, router_interface):
        """Should handle health check failure."""
        with patch.object(router_interface, 'execute_command', new_callable=AsyncMock) as mock_execute:
            mock_execute.side_effect = RouterConnectionError("Command failed")
            
            result = await router_interface.health_check()
            
            assert result == False

    # Parsing method tests
    def test_should_parse_csi_response(self, router_interface):
        """Should parse CSI response data."""
        mock_response = "CSI_DATA:timestamp,antennas,subcarriers,frequency,bandwidth"
        
        with patch('src.hardware.router_interface.CSIData') as mock_csi_data:
            expected_data = Mock(spec=CSIData)
            mock_csi_data.return_value = expected_data
            
            result = router_interface._parse_csi_response(mock_response)
            
            assert result == expected_data

    def test_should_parse_status_response(self, router_interface):
        """Should parse router status response."""
        mock_response = """
        cpu  123456 0 78901 234567 0 0 0 0 0 0
        MemTotal:     1024000 kB
        MemFree:       512000 kB
        wlan0     IEEE 802.11  ESSID:"TestNetwork"
        """
        
        result = router_interface._parse_status_response(mock_response)
        
        assert isinstance(result, dict)
        assert 'cpu_usage' in result
        assert 'memory_usage' in result
        assert 'wifi_status' in result