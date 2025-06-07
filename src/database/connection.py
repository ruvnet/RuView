"""
Database connection management for WiFi-DensePose API
"""

import asyncio
import logging
from typing import Optional, Dict, Any, AsyncGenerator
from contextlib import asynccontextmanager
from datetime import datetime

from sqlalchemy import create_engine, event, pool
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
        """Initialize PostgreSQL connections."""
        # Build database URL
        if self.settings.database_url:
            db_url = self.settings.database_url
            async_db_url = self.settings.database_url.replace("postgresql://", "postgresql+asyncpg://")
        else:
            db_url = (
                f"postgresql://{self.settings.db_user}:{self.settings.db_password}"
                f"@{self.settings.db_host}:{self.settings.db_port}/{self.settings.db_name}"
            )
            async_db_url = (
                f"postgresql+asyncpg://{self.settings.db_user}:{self.settings.db_password}"
                f"@{self.settings.db_host}:{self.settings.db_port}/{self.settings.db_name}"
            )
        
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
        
        logger.info("PostgreSQL connections initialized")
    
    async def _initialize_redis(self):
        """Initialize Redis connection."""
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
                raise
            else:
                logger.warning("Redis initialization failed but not required, continuing without Redis")
    
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
                result = await conn.execute("SELECT 1")
                await result.fetchone()
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
            "postgresql": {"status": "unknown", "details": {}},
            "redis": {"status": "unknown", "details": {}},
            "overall": "unknown"
        }
        
        # Check PostgreSQL
        try:
            start_time = datetime.utcnow()
            async with self.get_async_session() as session:
                result = await session.execute("SELECT 1")
                await result.fetchone()
            
            response_time = (datetime.utcnow() - start_time).total_seconds()
            
            health_status["postgresql"] = {
                "status": "healthy",
                "details": {
                    "response_time_ms": round(response_time * 1000, 2),
                    "pool_size": self._async_engine.pool.size(),
                    "checked_out": self._async_engine.pool.checkedout(),
                    "overflow": self._async_engine.pool.overflow(),
                }
            }
        except Exception as e:
            health_status["postgresql"] = {
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
        postgresql_healthy = health_status["postgresql"]["status"] == "healthy"
        redis_healthy = (
            health_status["redis"]["status"] in ["healthy", "disabled"] or
            not self.settings.redis_required
        )
        
        if postgresql_healthy and redis_healthy:
            health_status["overall"] = "healthy"
        elif postgresql_healthy:
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
                result = await session.execute("SELECT version()")
                version = (await result.fetchone())[0]
            
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