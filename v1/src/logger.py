"""
Logging configuration for WiFi-DensePose API
"""

import logging
import logging.config
import logging.handlers
import sys
import os
from pathlib import Path
from typing import Dict, Any, Optional
from datetime import datetime

from src.config.settings import Settings


class ColoredFormatter(logging.Formatter):
    """Colored log formatter for console output."""
    
    # ANSI color codes
    COLORS = {
        'DEBUG': '\033[36m',      # Cyan
        'INFO': '\033[32m',       # Green
        'WARNING': '\033[33m',    # Yellow
        'ERROR': '\033[31m',      # Red
        'CRITICAL': '\033[35m',   # Magenta
        'RESET': '\033[0m'        # Reset
    }
    
    def format(self, record):
        """Format log record with colors."""
        if hasattr(record, 'levelname'):
            color = self.COLORS.get(record.levelname, self.COLORS['RESET'])
            record.levelname = f"{color}{record.levelname}{self.COLORS['RESET']}"
        
        return super().format(record)


class StructuredFormatter(logging.Formatter):
    """Structured JSON formatter for log files."""
    
    def format(self, record):
        """Format log record as structured JSON."""
        import json
        
        log_entry = {
            'timestamp': datetime.utcnow().isoformat(),
            'level': record.levelname,
            'logger': record.name,
            'message': record.getMessage(),
            'module': record.module,
            'function': record.funcName,
            'line': record.lineno,
        }
        
        # Add exception info if present
        if record.exc_info:
            log_entry['exception'] = self.formatException(record.exc_info)
        
        # Add extra fields
        for key, value in record.__dict__.items():
            if key not in ['name', 'msg', 'args', 'levelname', 'levelno', 'pathname',
                          'filename', 'module', 'lineno', 'funcName', 'created',
                          'msecs', 'relativeCreated', 'thread', 'threadName',
                          'processName', 'process', 'getMessage', 'exc_info',
                          'exc_text', 'stack_info']:
                log_entry[key] = value
        
        return json.dumps(log_entry)


class RequestContextFilter(logging.Filter):
    """Filter to add request context to log records."""
    
    def filter(self, record):
        """Add request context to log record."""
        # Try to get request context from contextvars or thread local
        try:
            import contextvars
            request_id = contextvars.ContextVar('request_id', default=None).get()
            user_id = contextvars.ContextVar('user_id', default=None).get()
            
            if request_id:
                record.request_id = request_id
            if user_id:
                record.user_id = user_id
                
        except (ImportError, LookupError):
            pass
        
        return True


def setup_logging(settings: Settings) -> None:
    """Setup application logging configuration."""
    
    # Create log directory if file logging is enabled
    if settings.log_file:
        log_path = Path(settings.log_file)
        log_path.parent.mkdir(parents=True, exist_ok=True)
    
    # Build logging configuration
    config = build_logging_config(settings)
    
    # Apply configuration
    logging.config.dictConfig(config)
    
    # Set up root logger
    root_logger = logging.getLogger()
    root_logger.setLevel(settings.log_level)
    
    # Add request context filter to all handlers
    request_filter = RequestContextFilter()
    for handler in root_logger.handlers:
        handler.addFilter(request_filter)
    
    # Log startup message
    logger = logging.getLogger(__name__)
    logger.info(f"Logging configured - Level: {settings.log_level}, File: {settings.log_file}")


def build_logging_config(settings: Settings) -> Dict[str, Any]:
    """Build logging configuration dictionary."""
    
    config = {
        'version': 1,
        'disable_existing_loggers': False,
        'formatters': {
            'console': {
                '()': ColoredFormatter,
                'format': '%(asctime)s - %(name)s - %(levelname)s - %(message)s',
                'datefmt': '%Y-%m-%d %H:%M:%S'
            },
            'file': {
                'format': '%(asctime)s - %(name)s - %(levelname)s - %(module)s:%(lineno)d - %(message)s',
                'datefmt': '%Y-%m-%d %H:%M:%S'
            },
            'structured': {
                '()': StructuredFormatter
            }
        },
        'handlers': {
            'console': {
                'class': 'logging.StreamHandler',
                'level': settings.log_level,
                'formatter': 'console',
                'stream': 'ext://sys.stdout'
            }
        },
        'loggers': {
            '': {  # Root logger
                'level': settings.log_level,
                'handlers': ['console'],
                'propagate': False
            },
            'src': {  # Application logger
                'level': settings.log_level,
                'handlers': ['console'],
                'propagate': False
            },
            'uvicorn': {
                'level': 'INFO',
                'handlers': ['console'],
                'propagate': False
            },
            'uvicorn.access': {
                'level': 'INFO',
                'handlers': ['console'],
                'propagate': False
            },
            'fastapi': {
                'level': 'INFO',
                'handlers': ['console'],
                'propagate': False
            },
            'sqlalchemy': {
                'level': 'WARNING',
                'handlers': ['console'],
                'propagate': False
            },
            'sqlalchemy.engine': {
                'level': 'INFO' if settings.debug else 'WARNING',
                'handlers': ['console'],
                'propagate': False
            }
        }
    }
    
    # Add file handler if log file is specified
    if settings.log_file:
        config['handlers']['file'] = {
            'class': 'logging.handlers.RotatingFileHandler',
            'level': settings.log_level,
            'formatter': 'file',
            'filename': settings.log_file,
            'maxBytes': settings.log_max_size,
            'backupCount': settings.log_backup_count,
            'encoding': 'utf-8'
        }
        
        # Add structured log handler for JSON logs
        structured_log_file = str(Path(settings.log_file).with_suffix('.json'))
        config['handlers']['structured'] = {
            'class': 'logging.handlers.RotatingFileHandler',
            'level': settings.log_level,
            'formatter': 'structured',
            'filename': structured_log_file,
            'maxBytes': settings.log_max_size,
            'backupCount': settings.log_backup_count,
            'encoding': 'utf-8'
        }
        
        # Add file handlers to all loggers
        for logger_config in config['loggers'].values():
            logger_config['handlers'].extend(['file', 'structured'])
    
    return config


def get_logger(name: str) -> logging.Logger:
    """Get a logger with the specified name."""
    return logging.getLogger(name)


def configure_third_party_loggers(settings: Settings) -> None:
    """Configure third-party library loggers."""
    
    # Suppress noisy loggers in production
    if settings.is_production:
        logging.getLogger('urllib3').setLevel(logging.WARNING)
        logging.getLogger('requests').setLevel(logging.WARNING)
        logging.getLogger('asyncio').setLevel(logging.WARNING)
        logging.getLogger('multipart').setLevel(logging.WARNING)
    
    # Configure SQLAlchemy logging
    if settings.debug and settings.is_development:
        logging.getLogger('sqlalchemy.engine').setLevel(logging.INFO)
        logging.getLogger('sqlalchemy.pool').setLevel(logging.DEBUG)
    else:
        logging.getLogger('sqlalchemy').setLevel(logging.WARNING)
    
    # Configure Redis logging
    logging.getLogger('redis').setLevel(logging.WARNING)
    
    # Configure WebSocket logging
    logging.getLogger('websockets').setLevel(logging.INFO)


class LoggerMixin:
    """Mixin class to add logging capabilities to any class."""
    
    @property
    def logger(self) -> logging.Logger:
        """Get logger for this class."""
        return logging.getLogger(f"{self.__class__.__module__}.{self.__class__.__name__}")


def log_function_call(func):
    """Decorator to log function calls."""
    import functools
    
    @functools.wraps(func)
    def wrapper(*args, **kwargs):
        logger = logging.getLogger(func.__module__)
        logger.debug(f"Calling {func.__name__} with args={args}, kwargs={kwargs}")
        
        try:
            result = func(*args, **kwargs)
            logger.debug(f"{func.__name__} completed successfully")
            return result
        except Exception as e:
            logger.error(f"{func.__name__} failed with error: {e}")
            raise
    
    return wrapper


def log_async_function_call(func):
    """Decorator to log async function calls."""
    import functools
    
    @functools.wraps(func)
    async def wrapper(*args, **kwargs):
        logger = logging.getLogger(func.__module__)
        logger.debug(f"Calling async {func.__name__} with args={args}, kwargs={kwargs}")
        
        try:
            result = await func(*args, **kwargs)
            logger.debug(f"Async {func.__name__} completed successfully")
            return result
        except Exception as e:
            logger.error(f"Async {func.__name__} failed with error: {e}")
            raise
    
    return wrapper


def setup_request_logging():
    """Setup request-specific logging context."""
    import contextvars
    import uuid
    
    # Create context variables for request tracking
    request_id_var = contextvars.ContextVar('request_id')
    user_id_var = contextvars.ContextVar('user_id')
    
    def set_request_context(request_id: Optional[str] = None, user_id: Optional[str] = None):
        """Set request context for logging."""
        if request_id is None:
            request_id = str(uuid.uuid4())
        
        request_id_var.set(request_id)
        if user_id:
            user_id_var.set(user_id)
    
    def get_request_context():
        """Get current request context."""
        try:
            return {
                'request_id': request_id_var.get(),
                'user_id': user_id_var.get(None)
            }
        except LookupError:
            return {}
    
    return set_request_context, get_request_context


# Initialize request logging context
set_request_context, get_request_context = setup_request_logging()