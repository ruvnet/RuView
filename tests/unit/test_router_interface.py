import pytest
import numpy as np
from unittest.mock import Mock, patch, MagicMock
from src.hardware.router_interface import RouterInterface, RouterConnectionError


class TestRouterInterface:
    """Test suite for Router Interface following London School TDD principles"""
    
    @pytest.fixture
    def mock_config(self):
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
    def router_interface(self, mock_config):
        """Create router interface instance for testing"""
        return RouterInterface(mock_config)
    
    @pytest.fixture
    def mock_ssh_client(self):
        """Mock SSH client for testing"""
        mock_client = Mock()
        mock_client.connect = Mock()
        mock_client.exec_command = Mock()
        mock_client.close = Mock()
        return mock_client
    
    def test_interface_initialization_creates_correct_configuration(self, mock_config):
        """Test that router interface initializes with correct configuration"""
        # Act
        interface = RouterInterface(mock_config)
        
        # Assert
        assert interface is not None
        assert interface.router_ip == mock_config['router_ip']
        assert interface.username == mock_config['username']
        assert interface.password == mock_config['password']
        assert interface.ssh_port == mock_config['ssh_port']
        assert interface.timeout == mock_config['timeout']
        assert interface.max_retries == mock_config['max_retries']
        assert not interface.is_connected
    
    @patch('paramiko.SSHClient')
    def test_connect_establishes_ssh_connection(self, mock_ssh_class, router_interface, mock_ssh_client):
        """Test that connect method establishes SSH connection"""
        # Arrange
        mock_ssh_class.return_value = mock_ssh_client
        
        # Act
        result = router_interface.connect()
        
        # Assert
        assert result is True
        assert router_interface.is_connected is True
        mock_ssh_client.set_missing_host_key_policy.assert_called_once()
        mock_ssh_client.connect.assert_called_once_with(
            hostname=router_interface.router_ip,
            port=router_interface.ssh_port,
            username=router_interface.username,
            password=router_interface.password,
            timeout=router_interface.timeout
        )
    
    @patch('paramiko.SSHClient')
    def test_connect_handles_connection_failure(self, mock_ssh_class, router_interface, mock_ssh_client):
        """Test that connect method handles connection failures gracefully"""
        # Arrange
        mock_ssh_class.return_value = mock_ssh_client
        mock_ssh_client.connect.side_effect = Exception("Connection failed")
        
        # Act & Assert
        with pytest.raises(RouterConnectionError):
            router_interface.connect()
        
        assert router_interface.is_connected is False
    
    @patch('paramiko.SSHClient')
    def test_disconnect_closes_ssh_connection(self, mock_ssh_class, router_interface, mock_ssh_client):
        """Test that disconnect method closes SSH connection"""
        # Arrange
        mock_ssh_class.return_value = mock_ssh_client
        router_interface.connect()
        
        # Act
        router_interface.disconnect()
        
        # Assert
        assert router_interface.is_connected is False
        mock_ssh_client.close.assert_called_once()
    
    @patch('paramiko.SSHClient')
    def test_execute_command_runs_ssh_command(self, mock_ssh_class, router_interface, mock_ssh_client):
        """Test that execute_command runs SSH commands correctly"""
        # Arrange
        mock_ssh_class.return_value = mock_ssh_client
        mock_stdout = Mock()
        mock_stdout.read.return_value = b"command output"
        mock_stderr = Mock()
        mock_stderr.read.return_value = b""
        mock_ssh_client.exec_command.return_value = (None, mock_stdout, mock_stderr)
        
        router_interface.connect()
        
        # Act
        result = router_interface.execute_command("test command")
        
        # Assert
        assert result == "command output"
        mock_ssh_client.exec_command.assert_called_with("test command")
    
    @patch('paramiko.SSHClient')
    def test_execute_command_handles_command_errors(self, mock_ssh_class, router_interface, mock_ssh_client):
        """Test that execute_command handles command errors"""
        # Arrange
        mock_ssh_class.return_value = mock_ssh_client
        mock_stdout = Mock()
        mock_stdout.read.return_value = b""
        mock_stderr = Mock()
        mock_stderr.read.return_value = b"command error"
        mock_ssh_client.exec_command.return_value = (None, mock_stdout, mock_stderr)
        
        router_interface.connect()
        
        # Act & Assert
        with pytest.raises(RouterConnectionError):
            router_interface.execute_command("failing command")
    
    def test_execute_command_requires_connection(self, router_interface):
        """Test that execute_command requires active connection"""
        # Act & Assert
        with pytest.raises(RouterConnectionError):
            router_interface.execute_command("test command")
    
    @patch('paramiko.SSHClient')
    def test_get_router_info_retrieves_system_information(self, mock_ssh_class, router_interface, mock_ssh_client):
        """Test that get_router_info retrieves router system information"""
        # Arrange
        mock_ssh_class.return_value = mock_ssh_client
        mock_stdout = Mock()
        mock_stdout.read.return_value = b"Router Model: AC1900\nFirmware: 1.2.3"
        mock_stderr = Mock()
        mock_stderr.read.return_value = b""
        mock_ssh_client.exec_command.return_value = (None, mock_stdout, mock_stderr)
        
        router_interface.connect()
        
        # Act
        info = router_interface.get_router_info()
        
        # Assert
        assert info is not None
        assert isinstance(info, dict)
        assert 'model' in info
        assert 'firmware' in info
    
    @patch('paramiko.SSHClient')
    def test_enable_monitor_mode_configures_wifi_monitoring(self, mock_ssh_class, router_interface, mock_ssh_client):
        """Test that enable_monitor_mode configures WiFi monitoring"""
        # Arrange
        mock_ssh_class.return_value = mock_ssh_client
        mock_stdout = Mock()
        mock_stdout.read.return_value = b"Monitor mode enabled"
        mock_stderr = Mock()
        mock_stderr.read.return_value = b""
        mock_ssh_client.exec_command.return_value = (None, mock_stdout, mock_stderr)
        
        router_interface.connect()
        
        # Act
        result = router_interface.enable_monitor_mode("wlan0")
        
        # Assert
        assert result is True
        mock_ssh_client.exec_command.assert_called()
    
    @patch('paramiko.SSHClient')
    def test_disable_monitor_mode_disables_wifi_monitoring(self, mock_ssh_class, router_interface, mock_ssh_client):
        """Test that disable_monitor_mode disables WiFi monitoring"""
        # Arrange
        mock_ssh_class.return_value = mock_ssh_client
        mock_stdout = Mock()
        mock_stdout.read.return_value = b"Monitor mode disabled"
        mock_stderr = Mock()
        mock_stderr.read.return_value = b""
        mock_ssh_client.exec_command.return_value = (None, mock_stdout, mock_stderr)
        
        router_interface.connect()
        
        # Act
        result = router_interface.disable_monitor_mode("wlan0")
        
        # Assert
        assert result is True
        mock_ssh_client.exec_command.assert_called()
    
    @patch('paramiko.SSHClient')
    def test_interface_supports_context_manager(self, mock_ssh_class, router_interface, mock_ssh_client):
        """Test that router interface supports context manager protocol"""
        # Arrange
        mock_ssh_class.return_value = mock_ssh_client
        
        # Act
        with router_interface as interface:
            # Assert
            assert interface.is_connected is True
        
        # Assert - connection should be closed after context
        assert router_interface.is_connected is False
        mock_ssh_client.close.assert_called_once()
    
    def test_interface_validates_configuration(self):
        """Test that router interface validates configuration parameters"""
        # Arrange
        invalid_config = {
            'router_ip': '',  # Invalid IP
            'username': 'admin',
            'password': 'password'
        }
        
        # Act & Assert
        with pytest.raises(ValueError):
            RouterInterface(invalid_config)
    
    @patch('paramiko.SSHClient')
    def test_interface_implements_retry_logic(self, mock_ssh_class, router_interface, mock_ssh_client):
        """Test that interface implements retry logic for failed operations"""
        # Arrange
        mock_ssh_class.return_value = mock_ssh_client
        mock_ssh_client.connect.side_effect = [Exception("Temp failure"), None]  # Fail once, then succeed
        
        # Act
        result = router_interface.connect()
        
        # Assert
        assert result is True
        assert mock_ssh_client.connect.call_count == 2  # Should retry once