#!/usr/bin/env python3
"""
Main application entry point for WiFi-DensePose API
"""

import sys
import os
import asyncio
import logging
import signal
from pathlib import Path
from typing import Optional

# Add src to Python path
sys.path.insert(0, str(Path(__file__).parent))

from src.config.settings import get_settings, validate_settings
from src.logger import setup_logging
from src.app import create_app
from src.services.orchestrator import ServiceOrchestrator
from src.cli import create_cli


def setup_signal_handlers(orchestrator: ServiceOrchestrator):
    """Setup signal handlers for graceful shutdown."""
    def signal_handler(signum, frame):
        logging.info(f"Received signal {signum}, initiating graceful shutdown...")
        asyncio.create_task(orchestrator.shutdown())
        sys.exit(0)
    
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)


async def main():
    """Main application entry point."""
    try:
        # Load settings
        settings = get_settings()
        
        # Setup logging
        setup_logging(settings)
        logger = logging.getLogger(__name__)
        
        logger.info(f"Starting {settings.app_name} v{settings.version}")
        logger.info(f"Environment: {settings.environment}")
        
        # Validate settings
        issues = validate_settings(settings)
        if issues:
            logger.error("Configuration issues found:")
            for issue in issues:
                logger.error(f"  - {issue}")
            if settings.is_production:
                sys.exit(1)
            else:
                logger.warning("Continuing with configuration issues in development mode")
        
        # Create service orchestrator
        orchestrator = ServiceOrchestrator(settings)
        
        # Setup signal handlers
        setup_signal_handlers(orchestrator)
        
        # Initialize services
        await orchestrator.initialize()
        
        # Create FastAPI app
        app = create_app(settings, orchestrator)
        
        # Start the application
        if len(sys.argv) > 1:
            # CLI mode
            cli = create_cli(orchestrator)
            await cli.run(sys.argv[1:])
        else:
            # Server mode
            import uvicorn
            
            logger.info(f"Starting server on {settings.host}:{settings.port}")
            
            config = uvicorn.Config(
                app,
                host=settings.host,
                port=settings.port,
                reload=settings.reload and settings.is_development,
                workers=settings.workers if not settings.reload else 1,
                log_level=settings.log_level.lower(),
                access_log=True,
                use_colors=True
            )
            
            server = uvicorn.Server(config)
            await server.serve()
    
    except KeyboardInterrupt:
        logger.info("Received keyboard interrupt, shutting down...")
    except Exception as e:
        logger.error(f"Application failed to start: {e}", exc_info=True)
        sys.exit(1)
    finally:
        # Cleanup
        if 'orchestrator' in locals():
            await orchestrator.shutdown()
        logger.info("Application shutdown complete")


def run():
    """Entry point for package installation."""
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    run()