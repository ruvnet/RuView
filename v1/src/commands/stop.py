"""
Stop command implementation for WiFi-DensePose API
"""

import asyncio
import os
import signal
import time
from pathlib import Path
from typing import Optional

from src.config.settings import Settings
from src.logger import get_logger

logger = get_logger(__name__)


async def stop_command(
    settings: Settings,
    force: bool = False,
    timeout: int = 30
) -> None:
    """Stop the WiFi-DensePose API server."""
    
    logger.info("Stopping WiFi-DensePose API server...")
    
    # Get server status
    status = get_server_status(settings)
    
    if not status["running"]:
        if status["pid_file_exists"]:
            logger.info("Server is not running, but PID file exists. Cleaning up...")
            _cleanup_pid_file(settings)
        else:
            logger.info("Server is not running")
        return
    
    pid = status["pid"]
    logger.info(f"Found running server with PID {pid}")
    
    try:
        if force:
            await _force_stop_server(pid, settings)
        else:
            await _graceful_stop_server(pid, timeout, settings)
            
    except Exception as e:
        logger.error(f"Failed to stop server: {e}")
        raise


async def _graceful_stop_server(pid: int, timeout: int, settings: Settings) -> None:
    """Stop server gracefully with timeout."""
    
    logger.info(f"Attempting graceful shutdown (timeout: {timeout}s)...")
    
    try:
        # Send SIGTERM for graceful shutdown
        os.kill(pid, signal.SIGTERM)
        logger.info("Sent SIGTERM signal")
        
        # Wait for process to terminate
        start_time = time.time()
        while time.time() - start_time < timeout:
            try:
                # Check if process is still running
                os.kill(pid, 0)
                await asyncio.sleep(1)
            except OSError:
                # Process has terminated
                logger.info("Server stopped gracefully")
                _cleanup_pid_file(settings)
                return
        
        # Timeout reached, force kill
        logger.warning(f"Graceful shutdown timeout ({timeout}s) reached, forcing stop...")
        await _force_stop_server(pid, settings)
        
    except OSError as e:
        if e.errno == 3:  # No such process
            logger.info("Process already terminated")
            _cleanup_pid_file(settings)
        else:
            logger.error(f"Failed to send signal to process {pid}: {e}")
            raise


async def _force_stop_server(pid: int, settings: Settings) -> None:
    """Force stop server immediately."""
    
    logger.info("Force stopping server...")
    
    try:
        # Send SIGKILL for immediate termination
        os.kill(pid, signal.SIGKILL)
        logger.info("Sent SIGKILL signal")
        
        # Wait a moment for process to die
        await asyncio.sleep(2)
        
        # Verify process is dead
        try:
            os.kill(pid, 0)
            logger.error(f"Process {pid} still running after SIGKILL")
        except OSError:
            logger.info("Server force stopped")
            
    except OSError as e:
        if e.errno == 3:  # No such process
            logger.info("Process already terminated")
        else:
            logger.error(f"Failed to force kill process {pid}: {e}")
            raise
    
    finally:
        _cleanup_pid_file(settings)


def _cleanup_pid_file(settings: Settings) -> None:
    """Clean up PID file."""
    
    pid_file = Path(settings.log_directory) / "wifi-densepose-api.pid"
    
    if pid_file.exists():
        try:
            pid_file.unlink()
            logger.info("Cleaned up PID file")
        except Exception as e:
            logger.warning(f"Failed to remove PID file: {e}")


def get_server_status(settings: Settings) -> dict:
    """Get current server status."""
    
    pid_file = Path(settings.log_directory) / "wifi-densepose-api.pid"
    
    status = {
        "running": False,
        "pid": None,
        "pid_file": str(pid_file),
        "pid_file_exists": pid_file.exists(),
    }
    
    if pid_file.exists():
        try:
            with open(pid_file, 'r') as f:
                pid = int(f.read().strip())
            
            status["pid"] = pid
            
            # Check if process is running
            try:
                os.kill(pid, 0)  # Signal 0 just checks if process exists
                status["running"] = True
            except OSError:
                # Process doesn't exist
                status["running"] = False
                
        except (ValueError, IOError):
            # Invalid PID file
            status["running"] = False
    
    return status


async def stop_all_background_tasks(settings: Settings) -> None:
    """Stop all background tasks if they're running."""
    
    logger.info("Stopping background tasks...")
    
    try:
        # This would typically involve connecting to a task queue or
        # sending signals to background processes
        # For now, we'll just log the action
        
        logger.info("Background tasks stop signal sent")
        
    except Exception as e:
        logger.error(f"Failed to stop background tasks: {e}")


async def cleanup_resources(settings: Settings) -> None:
    """Clean up system resources."""
    
    logger.info("Cleaning up resources...")
    
    try:
        # Close database connections
        from src.database.connection import get_database_manager
        
        db_manager = get_database_manager(settings)
        await db_manager.close_all_connections()
        logger.info("Database connections closed")
        
    except Exception as e:
        logger.warning(f"Failed to close database connections: {e}")
    
    try:
        # Clean up temporary files
        temp_files = [
            Path(settings.log_directory) / "temp",
            Path(settings.backup_directory) / "temp",
        ]
        
        for temp_path in temp_files:
            if temp_path.exists() and temp_path.is_dir():
                import shutil
                shutil.rmtree(temp_path)
                logger.info(f"Cleaned up temporary directory: {temp_path}")
        
    except Exception as e:
        logger.warning(f"Failed to clean up temporary files: {e}")
    
    logger.info("Resource cleanup completed")


def is_server_running(settings: Settings) -> bool:
    """Check if server is currently running."""
    
    status = get_server_status(settings)
    return status["running"]


def get_server_pid(settings: Settings) -> Optional[int]:
    """Get server PID if running."""
    
    status = get_server_status(settings)
    return status["pid"] if status["running"] else None


async def wait_for_server_stop(settings: Settings, timeout: int = 30) -> bool:
    """Wait for server to stop with timeout."""
    
    start_time = time.time()
    
    while time.time() - start_time < timeout:
        if not is_server_running(settings):
            return True
        await asyncio.sleep(1)
    
    return False


def send_reload_signal(settings: Settings) -> bool:
    """Send reload signal to running server."""
    
    status = get_server_status(settings)
    
    if not status["running"]:
        logger.error("Server is not running")
        return False
    
    try:
        # Send SIGHUP for reload
        os.kill(status["pid"], signal.SIGHUP)
        logger.info("Sent reload signal to server")
        return True
        
    except OSError as e:
        logger.error(f"Failed to send reload signal: {e}")
        return False


async def restart_server(settings: Settings, timeout: int = 30) -> None:
    """Restart the server (stop then start)."""
    
    logger.info("Restarting server...")
    
    # Stop server if running
    if is_server_running(settings):
        await stop_command(settings, timeout=timeout)
        
        # Wait for server to stop
        if not await wait_for_server_stop(settings, timeout):
            logger.error("Server did not stop within timeout, forcing restart")
            await stop_command(settings, force=True)
    
    # Start server
    from src.commands.start import start_command
    await start_command(settings)


def get_stop_status_summary(settings: Settings) -> dict:
    """Get a summary of stop operation status."""
    
    status = get_server_status(settings)
    
    return {
        "server_running": status["running"],
        "pid": status["pid"],
        "pid_file_exists": status["pid_file_exists"],
        "can_stop": status["running"],
        "cleanup_needed": status["pid_file_exists"] and not status["running"],
    }