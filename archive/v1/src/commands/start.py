"""
Start command implementation for WiFi-DensePose API
"""

import asyncio
import os
import signal
import sys
import uvicorn
from pathlib import Path
from typing import Optional

from src.config.settings import Settings
from src.logger import get_logger

logger = get_logger(__name__)


async def start_command(
    settings: Settings,
    host: str = "0.0.0.0",
    port: int = 8000,
    workers: int = 1,
    reload: bool = False,
    daemon: bool = False
) -> None:
    """Start the WiFi-DensePose API server."""
    
    logger.info(f"Starting WiFi-DensePose API server...")
    logger.info(f"Environment: {settings.environment}")
    logger.info(f"Debug mode: {settings.debug}")
    logger.info(f"Host: {host}")
    logger.info(f"Port: {port}")
    logger.info(f"Workers: {workers}")
    
    # Validate settings
    await _validate_startup_requirements(settings)
    
    # Setup signal handlers
    _setup_signal_handlers()
    
    # Create PID file if running as daemon
    pid_file = None
    if daemon:
        pid_file = _create_pid_file(settings)
    
    try:
        # Initialize database
        await _initialize_database(settings)
        
        # Start background tasks
        background_tasks = await _start_background_tasks(settings)
        
        # Configure uvicorn
        uvicorn_config = {
            "app": "src.app:app",
            "host": host,
            "port": port,
            "reload": reload,
            "workers": workers if not reload else 1,  # Reload doesn't work with multiple workers
            "log_level": "debug" if settings.debug else "info",
            "access_log": True,
            "use_colors": not daemon,
        }
        
        if daemon:
            # Run as daemon
            await _run_as_daemon(uvicorn_config, pid_file)
        else:
            # Run in foreground
            await _run_server(uvicorn_config)
            
    except KeyboardInterrupt:
        logger.info("Received interrupt signal, shutting down...")
    except Exception as e:
        logger.error(f"Server startup failed: {e}")
        raise
    finally:
        # Cleanup
        if pid_file and pid_file.exists():
            pid_file.unlink()
        
        # Stop background tasks
        if 'background_tasks' in locals():
            await _stop_background_tasks(background_tasks)


async def _validate_startup_requirements(settings: Settings) -> None:
    """Validate that all startup requirements are met."""
    
    logger.info("Validating startup requirements...")
    
    # Check database connection
    try:
        from src.database.connection import get_database_manager
        
        db_manager = get_database_manager(settings)
        await db_manager.test_connection()
        logger.info("✓ Database connection validated")
        
    except Exception as e:
        logger.error(f"✗ Database connection failed: {e}")
        raise
    
    # Check Redis connection (if enabled)
    if settings.redis_enabled:
        try:
            redis_stats = await db_manager.get_connection_stats()
            if "redis" in redis_stats and not redis_stats["redis"].get("error"):
                logger.info("✓ Redis connection validated")
            else:
                logger.warning("⚠ Redis connection failed, continuing without Redis")
                
        except Exception as e:
            logger.warning(f"⚠ Redis connection failed: {e}, continuing without Redis")
    
    # Check required directories
    directories = [
        ("Log directory", settings.log_directory),
        ("Backup directory", settings.backup_directory),
    ]
    
    for name, directory in directories:
        path = Path(directory)
        path.mkdir(parents=True, exist_ok=True)
        logger.info(f"✓ {name} ready: {directory}")
    
    logger.info("All startup requirements validated")


async def _initialize_database(settings: Settings) -> None:
    """Initialize database connection and run migrations if needed."""
    
    logger.info("Initializing database...")
    
    try:
        from src.database.connection import get_database_manager
        
        db_manager = get_database_manager(settings)
        await db_manager.initialize()
        
        logger.info("Database initialized successfully")
        
    except Exception as e:
        logger.error(f"Database initialization failed: {e}")
        raise


async def _start_background_tasks(settings: Settings) -> dict:
    """Start background tasks."""
    
    logger.info("Starting background tasks...")
    
    tasks = {}
    
    try:
        # Start cleanup task
        if settings.cleanup_interval_seconds > 0:
            from src.tasks.cleanup import run_periodic_cleanup
            
            cleanup_task = asyncio.create_task(run_periodic_cleanup(settings))
            tasks['cleanup'] = cleanup_task
            logger.info("✓ Cleanup task started")
        
        # Start monitoring task
        if settings.monitoring_interval_seconds > 0:
            from src.tasks.monitoring import run_periodic_monitoring
            
            monitoring_task = asyncio.create_task(run_periodic_monitoring(settings))
            tasks['monitoring'] = monitoring_task
            logger.info("✓ Monitoring task started")
        
        # Start backup task
        if settings.backup_interval_seconds > 0:
            from src.tasks.backup import run_periodic_backup
            
            backup_task = asyncio.create_task(run_periodic_backup(settings))
            tasks['backup'] = backup_task
            logger.info("✓ Backup task started")
        
        logger.info(f"Started {len(tasks)} background tasks")
        return tasks
        
    except Exception as e:
        logger.error(f"Failed to start background tasks: {e}")
        # Cancel any started tasks
        for task in tasks.values():
            task.cancel()
        raise


async def _stop_background_tasks(tasks: dict) -> None:
    """Stop background tasks gracefully."""
    
    logger.info("Stopping background tasks...")
    
    # Cancel all tasks
    for name, task in tasks.items():
        if not task.done():
            logger.info(f"Stopping {name} task...")
            task.cancel()
    
    # Wait for tasks to complete
    if tasks:
        await asyncio.gather(*tasks.values(), return_exceptions=True)
    
    logger.info("Background tasks stopped")


def _setup_signal_handlers() -> None:
    """Setup signal handlers for graceful shutdown."""
    
    def signal_handler(signum, frame):
        logger.info(f"Received signal {signum}, initiating graceful shutdown...")
        # The actual shutdown will be handled by the main loop
        sys.exit(0)
    
    # Setup signal handlers
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)
    
    if hasattr(signal, 'SIGHUP'):
        signal.signal(signal.SIGHUP, signal_handler)


def _create_pid_file(settings: Settings) -> Path:
    """Create PID file for daemon mode."""
    
    pid_file = Path(settings.log_directory) / "wifi-densepose-api.pid"
    
    # Check if PID file already exists
    if pid_file.exists():
        try:
            with open(pid_file, 'r') as f:
                old_pid = int(f.read().strip())
            
            # Check if process is still running
            try:
                os.kill(old_pid, 0)  # Signal 0 just checks if process exists
                logger.error(f"Server already running with PID {old_pid}")
                sys.exit(1)
            except OSError:
                # Process doesn't exist, remove stale PID file
                pid_file.unlink()
                logger.info("Removed stale PID file")
                
        except (ValueError, IOError):
            # Invalid PID file, remove it
            pid_file.unlink()
            logger.info("Removed invalid PID file")
    
    # Write current PID
    with open(pid_file, 'w') as f:
        f.write(str(os.getpid()))
    
    logger.info(f"Created PID file: {pid_file}")
    return pid_file


async def _run_server(config: dict) -> None:
    """Run the server in foreground mode."""
    
    logger.info("Starting server in foreground mode...")
    
    # Create uvicorn server
    server = uvicorn.Server(uvicorn.Config(**config))
    
    # Run server
    await server.serve()


async def _run_as_daemon(config: dict, pid_file: Path) -> None:
    """Run the server as a daemon."""
    
    logger.info("Starting server in daemon mode...")
    
    # Fork process
    try:
        pid = os.fork()
        if pid > 0:
            # Parent process
            logger.info(f"Server started as daemon with PID {pid}")
            sys.exit(0)
    except OSError as e:
        logger.error(f"Fork failed: {e}")
        sys.exit(1)
    
    # Child process continues
    
    # Decouple from parent environment
    os.chdir("/")
    os.setsid()
    os.umask(0)
    
    # Second fork
    try:
        pid = os.fork()
        if pid > 0:
            # Exit second parent
            sys.exit(0)
    except OSError as e:
        logger.error(f"Second fork failed: {e}")
        sys.exit(1)
    
    # Update PID file with daemon PID
    with open(pid_file, 'w') as f:
        f.write(str(os.getpid()))
    
    # Redirect standard file descriptors
    sys.stdout.flush()
    sys.stderr.flush()
    
    # Redirect stdin, stdout, stderr to /dev/null
    with open('/dev/null', 'r') as f:
        os.dup2(f.fileno(), sys.stdin.fileno())
    
    with open('/dev/null', 'w') as f:
        os.dup2(f.fileno(), sys.stdout.fileno())
        os.dup2(f.fileno(), sys.stderr.fileno())
    
    # Create uvicorn server
    server = uvicorn.Server(uvicorn.Config(**config))
    
    # Run server
    await server.serve()


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