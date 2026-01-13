"""
Command-line interface for WiFi-DensePose API
"""

import asyncio
import click
import sys
from pathlib import Path
from typing import Optional

from src.config.settings import get_settings, load_settings_from_file
from src.logger import setup_logging, get_logger
from src.commands.start import start_command
from src.commands.stop import stop_command
from src.commands.status import status_command

# Get default settings and setup logging for CLI
settings = get_settings()
setup_logging(settings)
logger = get_logger(__name__)


def get_settings_with_config(config_file: Optional[str] = None):
    """Get settings with optional config file."""
    if config_file:
        return load_settings_from_file(config_file)
    else:
        return get_settings()


@click.group()
@click.option(
    '--config',
    '-c',
    type=click.Path(exists=True),
    help='Path to configuration file'
)
@click.option(
    '--verbose',
    '-v',
    is_flag=True,
    help='Enable verbose logging'
)
@click.option(
    '--debug',
    is_flag=True,
    help='Enable debug mode'
)
@click.pass_context
def cli(ctx, config: Optional[str], verbose: bool, debug: bool):
    """WiFi-DensePose API Command Line Interface."""
    
    # Ensure context object exists
    ctx.ensure_object(dict)
    
    # Store CLI options in context
    ctx.obj['config_file'] = config
    ctx.obj['verbose'] = verbose
    ctx.obj['debug'] = debug
    
    # Setup logging level
    if debug:
        import logging
        logging.getLogger().setLevel(logging.DEBUG)
        logger.debug("Debug mode enabled")
    elif verbose:
        import logging
        logging.getLogger().setLevel(logging.INFO)
        logger.info("Verbose mode enabled")


@cli.command()
@click.option(
    '--host',
    default='0.0.0.0',
    help='Host to bind to (default: 0.0.0.0)'
)
@click.option(
    '--port',
    default=8000,
    type=int,
    help='Port to bind to (default: 8000)'
)
@click.option(
    '--workers',
    default=1,
    type=int,
    help='Number of worker processes (default: 1)'
)
@click.option(
    '--reload',
    is_flag=True,
    help='Enable auto-reload for development'
)
@click.option(
    '--daemon',
    '-d',
    is_flag=True,
    help='Run as daemon (background process)'
)
@click.pass_context
def start(ctx, host: str, port: int, workers: int, reload: bool, daemon: bool):
    """Start the WiFi-DensePose API server."""
    
    try:
        # Get settings
        settings = get_settings_with_config(ctx.obj.get('config_file'))
        
        # Override settings with CLI options
        if ctx.obj.get('debug'):
            settings.debug = True
        
        # Run start command
        asyncio.run(start_command(
            settings=settings,
            host=host,
            port=port,
            workers=workers,
            reload=reload,
            daemon=daemon
        ))
        
    except KeyboardInterrupt:
        logger.info("Received interrupt signal, shutting down...")
        sys.exit(0)
    except Exception as e:
        logger.error(f"Failed to start server: {e}")
        sys.exit(1)


@cli.command()
@click.option(
    '--force',
    '-f',
    is_flag=True,
    help='Force stop without graceful shutdown'
)
@click.option(
    '--timeout',
    default=30,
    type=int,
    help='Timeout for graceful shutdown (default: 30 seconds)'
)
@click.pass_context
def stop(ctx, force: bool, timeout: int):
    """Stop the WiFi-DensePose API server."""
    
    try:
        # Get settings
        settings = get_settings_with_config(ctx.obj.get('config_file'))
        
        # Run stop command
        asyncio.run(stop_command(
            settings=settings,
            force=force,
            timeout=timeout
        ))
        
    except Exception as e:
        logger.error(f"Failed to stop server: {e}")
        sys.exit(1)


@cli.command()
@click.option(
    '--format',
    type=click.Choice(['text', 'json']),
    default='text',
    help='Output format (default: text)'
)
@click.option(
    '--detailed',
    is_flag=True,
    help='Show detailed status information'
)
@click.pass_context
def status(ctx, format: str, detailed: bool):
    """Show the status of the WiFi-DensePose API server."""
    
    try:
        # Get settings
        settings = get_settings_with_config(ctx.obj.get('config_file'))
        
        # Run status command
        asyncio.run(status_command(
            settings=settings,
            output_format=format,
            detailed=detailed
        ))
        
    except Exception as e:
        logger.error(f"Failed to get status: {e}")
        sys.exit(1)


@cli.group()
def db():
    """Database management commands."""
    pass


@db.command()
@click.option(
    '--url',
    help='Database URL (overrides config)'
)
@click.pass_context
def init(ctx, url: Optional[str]):
    """Initialize the database schema."""
    
    try:
        from src.database.connection import get_database_manager
        from alembic.config import Config
        from alembic import command
        import os
        
        # Get settings
        settings = get_settings_with_config(ctx.obj.get('config_file'))
        
        if url:
            settings.database_url = url
        
        # Initialize database
        db_manager = get_database_manager(settings)
        
        async def init_db():
            await db_manager.initialize()
            logger.info("Database initialized successfully")
        
        asyncio.run(init_db())
        
        # Run migrations if alembic.ini exists
        alembic_ini_path = "alembic.ini"
        if os.path.exists(alembic_ini_path):
            try:
                alembic_cfg = Config(alembic_ini_path)
                # Set the database URL in the config
                alembic_cfg.set_main_option("sqlalchemy.url", settings.get_database_url())
                command.upgrade(alembic_cfg, "head")
                logger.info("Database migrations applied successfully")
            except Exception as migration_error:
                logger.warning(f"Migration failed, but database is initialized: {migration_error}")
        else:
            logger.info("No alembic.ini found, skipping migrations")
        
    except Exception as e:
        logger.error(f"Failed to initialize database: {e}")
        sys.exit(1)


@db.command()
@click.option(
    '--revision',
    default='head',
    help='Target revision (default: head)'
)
@click.pass_context
def migrate(ctx, revision: str):
    """Run database migrations."""
    
    try:
        from alembic.config import Config
        from alembic import command
        
        # Run migrations
        alembic_cfg = Config("alembic.ini")
        command.upgrade(alembic_cfg, revision)
        logger.info(f"Database migrated to revision: {revision}")
        
    except Exception as e:
        logger.error(f"Failed to run migrations: {e}")
        sys.exit(1)


@db.command()
@click.option(
    '--steps',
    default=1,
    type=int,
    help='Number of steps to rollback (default: 1)'
)
@click.pass_context
def rollback(ctx, steps: int):
    """Rollback database migrations."""
    
    try:
        from alembic.config import Config
        from alembic import command
        
        # Rollback migrations
        alembic_cfg = Config("alembic.ini")
        command.downgrade(alembic_cfg, f"-{steps}")
        logger.info(f"Database rolled back {steps} step(s)")
        
    except Exception as e:
        logger.error(f"Failed to rollback database: {e}")
        sys.exit(1)


@cli.group()
def tasks():
    """Background task management commands."""
    pass


@tasks.command()
@click.option(
    '--task',
    type=click.Choice(['cleanup', 'monitoring', 'backup']),
    help='Specific task to run'
)
@click.pass_context
def run(ctx, task: Optional[str]):
    """Run background tasks."""
    
    try:
        from src.tasks.cleanup import get_cleanup_manager
        from src.tasks.monitoring import get_monitoring_manager
        from src.tasks.backup import get_backup_manager
        
        # Get settings
        settings = get_settings_with_config(ctx.obj.get('config_file'))
        
        async def run_tasks():
            if task == 'cleanup' or task is None:
                cleanup_manager = get_cleanup_manager(settings)
                result = await cleanup_manager.run_all_tasks()
                logger.info(f"Cleanup result: {result}")
            
            if task == 'monitoring' or task is None:
                monitoring_manager = get_monitoring_manager(settings)
                result = await monitoring_manager.run_all_tasks()
                logger.info(f"Monitoring result: {result}")
            
            if task == 'backup' or task is None:
                backup_manager = get_backup_manager(settings)
                result = await backup_manager.run_all_tasks()
                logger.info(f"Backup result: {result}")
        
        asyncio.run(run_tasks())
        
    except Exception as e:
        logger.error(f"Failed to run tasks: {e}")
        sys.exit(1)


@tasks.command()
@click.pass_context
def status(ctx):
    """Show background task status."""
    
    try:
        from src.tasks.cleanup import get_cleanup_manager
        from src.tasks.monitoring import get_monitoring_manager
        from src.tasks.backup import get_backup_manager
        import json
        
        # Get settings
        settings = get_settings_with_config(ctx.obj.get('config_file'))
        
        # Get task managers
        cleanup_manager = get_cleanup_manager(settings)
        monitoring_manager = get_monitoring_manager(settings)
        backup_manager = get_backup_manager(settings)
        
        # Collect status
        status_data = {
            "cleanup": cleanup_manager.get_stats(),
            "monitoring": monitoring_manager.get_stats(),
            "backup": backup_manager.get_stats(),
        }
        
        # Print status
        click.echo(json.dumps(status_data, indent=2))
        
    except Exception as e:
        logger.error(f"Failed to get task status: {e}")
        sys.exit(1)


@cli.group()
def config():
    """Configuration management commands."""
    pass


@config.command()
@click.pass_context
def show(ctx):
    """Show current configuration."""
    
    try:
        import json
        
        # Get settings
        settings = get_settings_with_config(ctx.obj.get('config_file'))
        
        # Convert settings to dict (excluding sensitive data)
        config_dict = {
            "app_name": settings.app_name,
            "version": settings.version,
            "environment": settings.environment,
            "debug": settings.debug,
            "host": settings.host,
            "port": settings.port,
            "api_prefix": settings.api_prefix,
            "docs_url": settings.docs_url,
            "redoc_url": settings.redoc_url,
            "log_level": settings.log_level,
            "log_file": settings.log_file,
            "data_storage_path": settings.data_storage_path,
            "model_storage_path": settings.model_storage_path,
            "temp_storage_path": settings.temp_storage_path,
            "wifi_interface": settings.wifi_interface,
            "csi_buffer_size": settings.csi_buffer_size,
            "pose_confidence_threshold": settings.pose_confidence_threshold,
            "stream_fps": settings.stream_fps,
            "websocket_ping_interval": settings.websocket_ping_interval,
            "features": {
                "authentication": settings.enable_authentication,
                "rate_limiting": settings.enable_rate_limiting,
                "websockets": settings.enable_websockets,
                "historical_data": settings.enable_historical_data,
                "real_time_processing": settings.enable_real_time_processing,
                "cors": settings.cors_enabled,
            }
        }
        
        click.echo(json.dumps(config_dict, indent=2))
        
    except Exception as e:
        logger.error(f"Failed to show configuration: {e}")
        sys.exit(1)


@config.command()
@click.pass_context
def validate(ctx):
    """Validate configuration."""
    
    try:
        # Get settings
        settings = get_settings_with_config(ctx.obj.get('config_file'))
        
        # Validate database connection
        from src.database.connection import get_database_manager
        
        async def validate_config():
            db_manager = get_database_manager(settings)
            
            try:
                await db_manager.test_connection()
                click.echo("✓ Database connection: OK")
            except Exception as e:
                click.echo(f"✗ Database connection: FAILED - {e}")
                return False
            
            # Validate Redis connection (if configured)
            redis_url = settings.get_redis_url()
            if redis_url:
                try:
                    import redis.asyncio as redis
                    redis_client = redis.from_url(redis_url)
                    await redis_client.ping()
                    click.echo("✓ Redis connection: OK")
                    await redis_client.close()
                except Exception as e:
                    click.echo(f"✗ Redis connection: FAILED - {e}")
                    return False
            else:
                click.echo("- Redis connection: NOT CONFIGURED")
            
            # Validate directories
            from pathlib import Path
            
            directories = [
                ("Data storage", settings.data_storage_path),
                ("Model storage", settings.model_storage_path),
                ("Temp storage", settings.temp_storage_path),
            ]
            
            for name, directory in directories:
                path = Path(directory)
                if path.exists() and path.is_dir():
                    click.echo(f"✓ {name}: OK")
                else:
                    try:
                        path.mkdir(parents=True, exist_ok=True)
                        click.echo(f"✓ {name}: CREATED - {directory}")
                    except Exception as e:
                        click.echo(f"✗ {name}: FAILED TO CREATE - {directory} ({e})")
                        return False
            
            click.echo("\n✓ Configuration validation passed")
            return True
        
        result = asyncio.run(validate_config())
        if not result:
            sys.exit(1)
        
    except Exception as e:
        logger.error(f"Failed to validate configuration: {e}")
        sys.exit(1)


@config.command()
@click.option(
    '--format',
    type=click.Choice(['text', 'json']),
    default='text',
    help='Output format (default: text)'
)
@click.pass_context
def failsafe(ctx, format: str):
    """Show failsafe status and configuration."""
    
    try:
        import json
        from src.database.connection import get_database_manager
        
        # Get settings
        settings = get_settings_with_config(ctx.obj.get('config_file'))
        
        async def check_failsafe_status():
            db_manager = get_database_manager(settings)
            
            # Initialize database to check current state
            try:
                await db_manager.initialize()
            except Exception as e:
                logger.warning(f"Database initialization failed: {e}")
            
            # Collect failsafe status
            failsafe_status = {
                "database": {
                    "failsafe_enabled": settings.enable_database_failsafe,
                    "using_sqlite_fallback": db_manager.is_using_sqlite_fallback(),
                    "sqlite_fallback_path": settings.sqlite_fallback_path,
                    "primary_database_url": settings.get_database_url() if not db_manager.is_using_sqlite_fallback() else None,
                },
                "redis": {
                    "failsafe_enabled": settings.enable_redis_failsafe,
                    "redis_enabled": settings.redis_enabled,
                    "redis_required": settings.redis_required,
                    "redis_available": db_manager.is_redis_available(),
                    "redis_url": settings.get_redis_url() if settings.redis_enabled else None,
                },
                "overall_status": "healthy"
            }
            
            # Determine overall status
            if failsafe_status["database"]["using_sqlite_fallback"] or not failsafe_status["redis"]["redis_available"]:
                failsafe_status["overall_status"] = "degraded"
            
            # Output results
            if format == 'json':
                click.echo(json.dumps(failsafe_status, indent=2))
            else:
                click.echo("=== Failsafe Status ===\n")
                
                # Database status
                click.echo("Database:")
                if failsafe_status["database"]["using_sqlite_fallback"]:
                    click.echo("  ⚠️  Using SQLite fallback database")
                    click.echo(f"     Path: {failsafe_status['database']['sqlite_fallback_path']}")
                else:
                    click.echo("  ✓  Using primary database (PostgreSQL)")
                
                click.echo(f"  Failsafe enabled: {'Yes' if failsafe_status['database']['failsafe_enabled'] else 'No'}")
                
                # Redis status
                click.echo("\nRedis:")
                if not failsafe_status["redis"]["redis_enabled"]:
                    click.echo("  -  Redis disabled")
                elif not failsafe_status["redis"]["redis_available"]:
                    click.echo("  ⚠️  Redis unavailable (failsafe active)")
                else:
                    click.echo("  ✓  Redis available")
                
                click.echo(f"  Failsafe enabled: {'Yes' if failsafe_status['redis']['failsafe_enabled'] else 'No'}")
                click.echo(f"  Required: {'Yes' if failsafe_status['redis']['redis_required'] else 'No'}")
                
                # Overall status
                status_icon = "✓" if failsafe_status["overall_status"] == "healthy" else "⚠️"
                click.echo(f"\nOverall Status: {status_icon} {failsafe_status['overall_status'].upper()}")
                
                if failsafe_status["overall_status"] == "degraded":
                    click.echo("\nNote: System is running in degraded mode using failsafe configurations.")
        
        asyncio.run(check_failsafe_status())
        
    except Exception as e:
        logger.error(f"Failed to check failsafe status: {e}")
        sys.exit(1)


@cli.command()
def version():
    """Show version information."""
    
    try:
        from src.config.settings import get_settings
        
        settings = get_settings()
        
        click.echo(f"WiFi-DensePose API v{settings.version}")
        click.echo(f"Environment: {settings.environment}")
        click.echo(f"Python: {sys.version}")
        
    except Exception as e:
        logger.error(f"Failed to get version: {e}")
        sys.exit(1)


def create_cli(orchestrator=None):
    """Create CLI interface for the application."""
    return cli


if __name__ == '__main__':
    cli()