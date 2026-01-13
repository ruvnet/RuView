"""
Database connection management for WiFi-DensePose API
"""

import asyncio
import logging
from typing import Optional, Dict, Any, AsyncGenerator
from contextlib import asynccontextmanager
from datetime import datetime

from sqlalchemy import create_engine, event, pool, text
from sqlalchemy.ext.asyncio import create_async_engine, AsyncSession, async_sessionmaker
from sqlalchemy.orm import sessionmaker, Session
from sqlalchemy.pool import QueuePool, NullPool
from sqlalchemy.exc import SQLAlchemyError, DisconnectionError
import redis.asyncio as redis
from redis.exceptions import ConnectionError as RedisConnectionError

from src.config.settings import Settings
from src.logger import get_logger

logger = get_logger(__name__)


class DatabaseConnectionError(Exception):
    """Database connection error."""
    pass


class DatabaseManager:
    """Database connection manager."""
    
    def __init__(self, settings: Settings):
        self.settings = settings
        self._async_engine = None
        self._sync_engine = None
        self._async_session_factory = None
        self._sync_session_factory = None
        self._redis_client = None
        self._initialized = False
        self._connection_pool_size = settings.db_pool_size
        self._max_overflow = settings.db_max_overflow
        self._pool_timeout = settings.db_pool_timeout
        self._pool_recycle = settings.db_pool_recycle
    
    async def initialize(self):
        """Initialize database connections."""
        if self._initialized:
            return
        
        logger.info("Initializing database connections")
        
        try:
            # Initialize PostgreSQL connections
            await self._initialize_postgresql()
            
            # Initialize Redis connection
            await self._initialize_redis()
            
            self._initialized = True
            logger.info("Database connections initialized successfully")
            
        except Exception as e:
            logger.error(f"Failed to initialize database connections: {e}")
            raise DatabaseConnectionError(f"Database initialization failed: {e}")
    
    async def _initialize_postgresql(self):
        """Initialize PostgreSQL connections with SQLite failsafe."""
        postgresql_failed = False
        
        try:
            # Try PostgreSQL first
            await self._initialize_postgresql_primary()
            logger.info("PostgreSQL connections initialized")
            return
        except Exception as e:
            postgresql_failed = True
            logger.error(f"PostgreSQL initialization failed: {e}")
            
            if not self.settings.enable_database_failsafe:
                raise DatabaseConnectionError(f"PostgreSQL connection failed and failsafe disabled: {e}")
            
            logger.warning("Falling back to SQLite database")
        
        # Fallback to SQLite if PostgreSQL failed and failsafe is enabled
        if postgresql_failed and self.settings.enable_database_failsafe:
            await self._initialize_sqlite_fallback()
            logger.info("SQLite fallback database initialized")
    
    async def _initialize_postgresql_primary(self):
        """Initialize primary PostgreSQL connections."""
        # Build database URL
        if self.settings.database_url and "postgresql" in self.settings.database_url:
            db_url = self.settings.database_url
            async_db_url = self.settings.database_url.replace("postgresql://", "postgresql+asyncpg://")
        elif self.settings.db_host and self.settings.db_name and self.settings.db_user:
            db_url = (
                f"postgresql://{self.settings.db_user}:{self.settings.db_password}"
                f"@{self.settings.db_host}:{self.settings.db_port}/{self.settings.db_name}"
            )
            async_db_url = (
                f"postgresql+asyncpg://{self.settings.db_user}:{self.settings.db_password}"
                f"@{self.settings.db_host}:{self.settings.db_port}/{self.settings.db_name}"
            )
        else:
            raise ValueError("PostgreSQL connection parameters not configured")
        
        # Create async engine (don't specify poolclass for async engines)
        self._async_engine = create_async_engine(
            async_db_url,
            pool_size=self._connection_pool_size,
            max_overflow=self._max_overflow,
            pool_timeout=self._pool_timeout,
            pool_recycle=self._pool_recycle,
            pool_pre_ping=True,
            echo=self.settings.db_echo,
            future=True,
        )
        
        # Create sync engine for migrations and admin tasks
        self._sync_engine = create_engine(
            db_url,
            poolclass=QueuePool,
            pool_size=max(2, self._connection_pool_size // 2),
            max_overflow=self._max_overflow // 2,
            pool_timeout=self._pool_timeout,
            pool_recycle=self._pool_recycle,
            pool_pre_ping=True,
            echo=self.settings.db_echo,
            future=True,
        )
        
        # Create session factories
        self._async_session_factory = async_sessionmaker(
            self._async_engine,
            class_=AsyncSession,
            expire_on_commit=False,
        )
        
        self._sync_session_factory = sessionmaker(
            self._sync_engine,
            expire_on_commit=False,
        )
        
        # Add connection event listeners
        self._setup_connection_events()
        
        # Test connections
        await self._test_postgresql_connection()
    
    async def _initialize_sqlite_fallback(self):
        """Initialize SQLite fallback database."""
        import os
        
        # Ensure directory exists
        sqlite_path = self.settings.sqlite_fallback_path
        os.makedirs(os.path.dirname(sqlite_path), exist_ok=True)
        
        # Build SQLite URLs
        db_url = f"sqlite:///{sqlite_path}"
        async_db_url = f"sqlite+aiosqlite:///{sqlite_path}"
        
        # Create async engine for SQLite
        self._async_engine = create_async_engine(
            async_db_url,
            echo=self.settings.db_echo,
            future=True,
        )
        
        # Create sync engine for SQLite
        self._sync_engine = create_engine(
            db_url,
            poolclass=NullPool,  # SQLite doesn't need connection pooling
            echo=self.settings.db_echo,
            future=True,
        )
        
        # Create session factories
        self._async_session_factory = async_sessionmaker(
            self._async_engine,
            class_=AsyncSession,
            expire_on_commit=False,
        )
        
        self._sync_session_factory = sessionmaker(
            self._sync_engine,
            expire_on_commit=False,
        )
        
        # Add connection event listeners
        self._setup_connection_events()
        
        # Test SQLite connection
        await self._test_sqlite_connection()
    
    async def _test_sqlite_connection(self):
        """Test SQLite connection."""
        try:
            async with self._async_engine.begin() as conn:
                result = await conn.execute(text("SELECT 1"))
                result.fetchone()  # Don't await this - fetchone() is not async
            logger.debug("SQLite connection test successful")
        except Exception as e:
            logger.error(f"SQLite connection test failed: {e}")
            raise DatabaseConnectionError(f"SQLite connection test failed: {e}")
    
    async def _initialize_redis(self):
        """Initialize Redis connection with failsafe."""
        if not self.settings.redis_enabled:
            logger.info("Redis disabled, skipping initialization")
            return
        
        try:
            # Build Redis URL
            if self.settings.redis_url:
                redis_url = self.settings.redis_url
            else:
                redis_url = (
                    f"redis://{self.settings.redis_host}:{self.settings.redis_port}"
                    f"/{self.settings.redis_db}"
                )
            
            # Create Redis client
            self._redis_client = redis.from_url(
                redis_url,
                password=self.settings.redis_password,
                encoding="utf-8",
                decode_responses=True,
                max_connections=self.settings.redis_max_connections,
                retry_on_timeout=True,
                socket_timeout=self.settings.redis_socket_timeout,
                socket_connect_timeout=self.settings.redis_connect_timeout,
            )
            
            # Test Redis connection
            await self._test_redis_connection()
            
            logger.info("Redis connection initialized")
            
        except Exception as e:
            logger.error(f"Failed to initialize Redis: {e}")
            
            if self.settings.redis_required:
                raise DatabaseConnectionError(f"Redis connection failed and is required: {e}")
            elif self.settings.enable_redis_failsafe:
                logger.warning("Redis initialization failed, continuing without Redis (failsafe enabled)")
                self._redis_client = None
            else:
                logger.warning("Redis initialization failed but not required, continuing without Redis")
                self._redis_client = None
    
    def _setup_connection_events(self):
        """Setup database connection event listeners."""
        
        @event.listens_for(self._sync_engine, "connect")
        def set_sqlite_pragma(dbapi_connection, connection_record):
            """Set database-specific settings on connection."""
            if "sqlite" in str(self._sync_engine.url):
                cursor = dbapi_connection.cursor()
                cursor.execute("PRAGMA foreign_keys=ON")
                cursor.close()
        
        @event.listens_for(self._sync_engine, "checkout")
        def receive_checkout(dbapi_connection, connection_record, connection_proxy):
            """Log connection checkout."""
            logger.debug("Database connection checked out")
        
        @event.listens_for(self._sync_engine, "checkin")
        def receive_checkin(dbapi_connection, connection_record):
            """Log connection checkin."""
            logger.debug("Database connection checked in")
        
        @event.listens_for(self._sync_engine, "invalidate")
        def receive_invalidate(dbapi_connection, connection_record, exception):
            """Handle connection invalidation."""
            logger.warning(f"Database connection invalidated: {exception}")
    
    async def _test_postgresql_connection(self):
        """Test PostgreSQL connection."""
        try:
            async with self._async_engine.begin() as conn:
                result = await conn.execute(text("SELECT 1"))
                result.fetchone()  # Don't await this - fetchone() is not async
            logger.debug("PostgreSQL connection test successful")
        except Exception as e:
            logger.error(f"PostgreSQL connection test failed: {e}")
            raise DatabaseConnectionError(f"PostgreSQL connection test failed: {e}")
    
    async def _test_redis_connection(self):
        """Test Redis connection."""
        if not self._redis_client:
            return
        
        try:
            await self._redis_client.ping()
            logger.debug("Redis connection test successful")
        except Exception as e:
            logger.error(f"Redis connection test failed: {e}")
            if self.settings.redis_required:
                raise DatabaseConnectionError(f"Redis connection test failed: {e}")
    
    @asynccontextmanager
    async def get_async_session(self) -> AsyncGenerator[AsyncSession, None]:
        """Get async database session."""
        if not self._initialized:
            await self.initialize()
        
        if not self._async_session_factory:
            raise DatabaseConnectionError("Async session factory not initialized")
        
        session = self._async_session_factory()
        try:
            yield session
            await session.commit()
        except Exception as e:
            await session.rollback()
            logger.error(f"Database session error: {e}")
            raise
        finally:
            await session.close()
    
    @asynccontextmanager
    async def get_sync_session(self) -> Session:
        """Get sync database session."""
        if not self._initialized:
            await self.initialize()
        
        if not self._sync_session_factory:
            raise DatabaseConnectionError("Sync session factory not initialized")
        
        session = self._sync_session_factory()
        try:
            yield session
            session.commit()
        except Exception as e:
            session.rollback()
            logger.error(f"Database session error: {e}")
            raise
        finally:
            session.close()
    
    async def get_redis_client(self) -> Optional[redis.Redis]:
        """Get Redis client."""
        if not self._initialized:
            await self.initialize()
        
        return self._redis_client
    
    async def health_check(self) -> Dict[str, Any]:
        """Perform database health check."""
        health_status = {
            "database": {"status": "unknown", "details": {}},
            "redis": {"status": "unknown", "details": {}},
            "overall": "unknown"
        }
        
        # Check Database (PostgreSQL or SQLite)
        try:
            start_time = datetime.utcnow()
            async with self.get_async_session() as session:
                result = await session.execute(text("SELECT 1"))
                result.fetchone()  # Don't await this - fetchone() is not async
            
            response_time = (datetime.utcnow() - start_time).total_seconds()
            
            # Determine database type and status
            is_sqlite = self.is_using_sqlite_fallback()
            db_type = "sqlite_fallback" if is_sqlite else "postgresql"
            
            details = {
                "type": db_type,
                "response_time_ms": round(response_time * 1000, 2),
            }
            
            # Add pool info for PostgreSQL
            if not is_sqlite and hasattr(self._async_engine, 'pool'):
                details.update({
                    "pool_size": self._async_engine.pool.size(),
                    "checked_out": self._async_engine.pool.checkedout(),
                    "overflow": self._async_engine.pool.overflow(),
                })
            
            # Add failsafe info
            if is_sqlite:
                details["failsafe_active"] = True
                details["fallback_path"] = self.settings.sqlite_fallback_path
            
            health_status["database"] = {
                "status": "healthy",
                "details": details
            }
        except Exception as e:
            health_status["database"] = {
                "status": "unhealthy",
                "details": {"error": str(e)}
            }
        
        # Check Redis
        if self._redis_client:
            try:
                start_time = datetime.utcnow()
                await self._redis_client.ping()
                response_time = (datetime.utcnow() - start_time).total_seconds()
                
                info = await self._redis_client.info()
                
                health_status["redis"] = {
                    "status": "healthy",
                    "details": {
                        "response_time_ms": round(response_time * 1000, 2),
                        "connected_clients": info.get("connected_clients", 0),
                        "used_memory": info.get("used_memory_human", "unknown"),
                        "uptime": info.get("uptime_in_seconds", 0),
                    }
                }
            except Exception as e:
                health_status["redis"] = {
                    "status": "unhealthy",
                    "details": {"error": str(e)}
                }
        else:
            health_status["redis"] = {
                "status": "disabled",
                "details": {"message": "Redis not enabled"}
            }
        
        # Determine overall status
        database_healthy = health_status["database"]["status"] == "healthy"
        redis_healthy = (
            health_status["redis"]["status"] in ["healthy", "disabled"] or
            not self.settings.redis_required
        )
        
        # Check if using failsafe modes
        using_sqlite_fallback = self.is_using_sqlite_fallback()
        redis_unavailable = not self.is_redis_available() and self.settings.redis_enabled
        
        if database_healthy and redis_healthy:
            if using_sqlite_fallback or redis_unavailable:
                health_status["overall"] = "degraded"  # Working but using failsafe
            else:
                health_status["overall"] = "healthy"
        elif database_healthy:
            health_status["overall"] = "degraded"
        else:
            health_status["overall"] = "unhealthy"
        
        return health_status
    
    async def get_connection_stats(self) -> Dict[str, Any]:
        """Get database connection statistics."""
        stats = {
            "postgresql": {},
            "redis": {}
        }
        
        # PostgreSQL stats
        if self._async_engine:
            pool = self._async_engine.pool
            stats["postgresql"] = {
                "pool_size": pool.size(),
                "checked_out": pool.checkedout(),
                "overflow": pool.overflow(),
                "checked_in": pool.checkedin(),
                "total_connections": pool.size() + pool.overflow(),
                "available_connections": pool.size() - pool.checkedout(),
            }
        
        # Redis stats
        if self._redis_client:
            try:
                info = await self._redis_client.info()
                stats["redis"] = {
                    "connected_clients": info.get("connected_clients", 0),
                    "blocked_clients": info.get("blocked_clients", 0),
                    "total_connections_received": info.get("total_connections_received", 0),
                    "rejected_connections": info.get("rejected_connections", 0),
                }
            except Exception as e:
                stats["redis"] = {"error": str(e)}
        
        return stats
    
    async def close_connections(self):
        """Close all database connections."""
        logger.info("Closing database connections")
        
        # Close PostgreSQL connections
        if self._async_engine:
            await self._async_engine.dispose()
            logger.debug("Async PostgreSQL engine disposed")
        
        if self._sync_engine:
            self._sync_engine.dispose()
            logger.debug("Sync PostgreSQL engine disposed")
        
        # Close Redis connection
        if self._redis_client:
            await self._redis_client.close()
            logger.debug("Redis connection closed")
        
        self._initialized = False
        logger.info("Database connections closed")
    
    def is_using_sqlite_fallback(self) -> bool:
        """Check if currently using SQLite fallback database."""
        if not self._async_engine:
            return False
        return "sqlite" in str(self._async_engine.url)
    
    def is_redis_available(self) -> bool:
        """Check if Redis is available."""
        return self._redis_client is not None
    
    async def test_connection(self) -> bool:
        """Test database connection for CLI validation."""
        try:
            if not self._initialized:
                await self.initialize()
            
            # Test database connection (PostgreSQL or SQLite)
            async with self.get_async_session() as session:
                result = await session.execute(text("SELECT 1"))
                result.fetchone()  # Don't await this - fetchone() is not async
            
            # Test Redis connection if enabled
            if self._redis_client:
                await self._redis_client.ping()
            
            return True
        except Exception as e:
            logger.error(f"Database connection test failed: {e}")
            return False
    
    async def reset_connections(self):
        """Reset all database connections."""
        logger.info("Resetting database connections")
        await self.close_connections()
        await self.initialize()
        logger.info("Database connections reset")


# Global database manager instance
_db_manager: Optional[DatabaseManager] = None


def get_database_manager(settings: Settings) -> DatabaseManager:
    """Get database manager instance."""
    global _db_manager
    if _db_manager is None:
        _db_manager = DatabaseManager(settings)
    return _db_manager


async def get_async_session(settings: Settings) -> AsyncGenerator[AsyncSession, None]:
    """Dependency to get async database session."""
    db_manager = get_database_manager(settings)
    async with db_manager.get_async_session() as session:
        yield session


async def get_redis_client(settings: Settings) -> Optional[redis.Redis]:
    """Dependency to get Redis client."""
    db_manager = get_database_manager(settings)
    return await db_manager.get_redis_client()


class DatabaseHealthCheck:
    """Database health check utility."""
    
    def __init__(self, db_manager: DatabaseManager):
        self.db_manager = db_manager
    
    async def check_postgresql(self) -> Dict[str, Any]:
        """Check PostgreSQL health."""
        try:
            start_time = datetime.utcnow()
            async with self.db_manager.get_async_session() as session:
                result = await session.execute(text("SELECT version()"))
                version = result.fetchone()[0]  # Don't await this - fetchone() is not async
            
            response_time = (datetime.utcnow() - start_time).total_seconds()
            
            return {
                "status": "healthy",
                "version": version,
                "response_time_ms": round(response_time * 1000, 2),
            }
        except Exception as e:
            return {
                "status": "unhealthy",
                "error": str(e),
            }
    
    async def check_redis(self) -> Dict[str, Any]:
        """Check Redis health."""
        redis_client = await self.db_manager.get_redis_client()
        
        if not redis_client:
            return {
                "status": "disabled",
                "message": "Redis not configured"
            }
        
        try:
            start_time = datetime.utcnow()
            pong = await redis_client.ping()
            response_time = (datetime.utcnow() - start_time).total_seconds()
            
            info = await redis_client.info("server")
            
            return {
                "status": "healthy",
                "ping": pong,
                "version": info.get("redis_version", "unknown"),
                "response_time_ms": round(response_time * 1000, 2),
            }
        except Exception as e:
            return {
                "status": "unhealthy",
                "error": str(e),
            }
    
    async def full_health_check(self) -> Dict[str, Any]:
        """Perform full database health check."""
        postgresql_health = await self.check_postgresql()
        redis_health = await self.check_redis()
        
        overall_status = "healthy"
        if postgresql_health["status"] != "healthy":
            overall_status = "unhealthy"
        elif redis_health["status"] == "unhealthy":
            overall_status = "degraded"
        
        return {
            "overall_status": overall_status,
            "postgresql": postgresql_health,
            "redis": redis_health,
            "timestamp": datetime.utcnow().isoformat(),
        }