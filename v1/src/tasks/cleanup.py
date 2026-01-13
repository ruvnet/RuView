"""
Periodic cleanup tasks for WiFi-DensePose API
"""

import asyncio
import logging
from datetime import datetime, timedelta
from typing import Dict, Any, Optional, List
from contextlib import asynccontextmanager

from sqlalchemy import delete, select, func, and_, or_
from sqlalchemy.ext.asyncio import AsyncSession

from src.config.settings import Settings
from src.database.connection import get_database_manager
from src.database.models import (
    CSIData, PoseDetection, SystemMetric, AuditLog, Session, Device
)
from src.logger import get_logger

logger = get_logger(__name__)


class CleanupTask:
    """Base class for cleanup tasks."""
    
    def __init__(self, name: str, settings: Settings):
        self.name = name
        self.settings = settings
        self.enabled = True
        self.last_run = None
        self.run_count = 0
        self.error_count = 0
        self.total_cleaned = 0
    
    async def execute(self, session: AsyncSession) -> Dict[str, Any]:
        """Execute the cleanup task."""
        raise NotImplementedError
    
    async def run(self, session: AsyncSession) -> Dict[str, Any]:
        """Run the cleanup task with error handling."""
        start_time = datetime.utcnow()
        
        try:
            logger.info(f"Starting cleanup task: {self.name}")
            
            result = await self.execute(session)
            
            self.last_run = start_time
            self.run_count += 1
            
            if result.get("cleaned_count", 0) > 0:
                self.total_cleaned += result["cleaned_count"]
                logger.info(
                    f"Cleanup task {self.name} completed: "
                    f"cleaned {result['cleaned_count']} items"
                )
            else:
                logger.debug(f"Cleanup task {self.name} completed: no items to clean")
            
            return {
                "task": self.name,
                "status": "success",
                "start_time": start_time.isoformat(),
                "duration_ms": (datetime.utcnow() - start_time).total_seconds() * 1000,
                **result
            }
            
        except Exception as e:
            self.error_count += 1
            logger.error(f"Cleanup task {self.name} failed: {e}", exc_info=True)
            
            return {
                "task": self.name,
                "status": "error",
                "start_time": start_time.isoformat(),
                "duration_ms": (datetime.utcnow() - start_time).total_seconds() * 1000,
                "error": str(e),
                "cleaned_count": 0
            }
    
    def get_stats(self) -> Dict[str, Any]:
        """Get task statistics."""
        return {
            "name": self.name,
            "enabled": self.enabled,
            "last_run": self.last_run.isoformat() if self.last_run else None,
            "run_count": self.run_count,
            "error_count": self.error_count,
            "total_cleaned": self.total_cleaned,
        }


class OldCSIDataCleanup(CleanupTask):
    """Cleanup old CSI data records."""
    
    def __init__(self, settings: Settings):
        super().__init__("old_csi_data_cleanup", settings)
        self.retention_days = settings.csi_data_retention_days
        self.batch_size = settings.cleanup_batch_size
    
    async def execute(self, session: AsyncSession) -> Dict[str, Any]:
        """Execute CSI data cleanup."""
        if self.retention_days <= 0:
            return {"cleaned_count": 0, "message": "CSI data retention disabled"}
        
        cutoff_date = datetime.utcnow() - timedelta(days=self.retention_days)
        
        # Count records to be deleted
        count_query = select(func.count(CSIData.id)).where(
            CSIData.created_at < cutoff_date
        )
        total_count = await session.scalar(count_query)
        
        if total_count == 0:
            return {"cleaned_count": 0, "message": "No old CSI data to clean"}
        
        # Delete in batches
        cleaned_count = 0
        while cleaned_count < total_count:
            # Get batch of IDs to delete
            id_query = select(CSIData.id).where(
                CSIData.created_at < cutoff_date
            ).limit(self.batch_size)
            
            result = await session.execute(id_query)
            ids_to_delete = [row[0] for row in result.fetchall()]
            
            if not ids_to_delete:
                break
            
            # Delete batch
            delete_query = delete(CSIData).where(CSIData.id.in_(ids_to_delete))
            await session.execute(delete_query)
            await session.commit()
            
            batch_size = len(ids_to_delete)
            cleaned_count += batch_size
            
            logger.debug(f"Deleted {batch_size} CSI data records (total: {cleaned_count})")
            
            # Small delay to avoid overwhelming the database
            await asyncio.sleep(0.1)
        
        return {
            "cleaned_count": cleaned_count,
            "retention_days": self.retention_days,
            "cutoff_date": cutoff_date.isoformat()
        }


class OldPoseDetectionCleanup(CleanupTask):
    """Cleanup old pose detection records."""
    
    def __init__(self, settings: Settings):
        super().__init__("old_pose_detection_cleanup", settings)
        self.retention_days = settings.pose_detection_retention_days
        self.batch_size = settings.cleanup_batch_size
    
    async def execute(self, session: AsyncSession) -> Dict[str, Any]:
        """Execute pose detection cleanup."""
        if self.retention_days <= 0:
            return {"cleaned_count": 0, "message": "Pose detection retention disabled"}
        
        cutoff_date = datetime.utcnow() - timedelta(days=self.retention_days)
        
        # Count records to be deleted
        count_query = select(func.count(PoseDetection.id)).where(
            PoseDetection.created_at < cutoff_date
        )
        total_count = await session.scalar(count_query)
        
        if total_count == 0:
            return {"cleaned_count": 0, "message": "No old pose detections to clean"}
        
        # Delete in batches
        cleaned_count = 0
        while cleaned_count < total_count:
            # Get batch of IDs to delete
            id_query = select(PoseDetection.id).where(
                PoseDetection.created_at < cutoff_date
            ).limit(self.batch_size)
            
            result = await session.execute(id_query)
            ids_to_delete = [row[0] for row in result.fetchall()]
            
            if not ids_to_delete:
                break
            
            # Delete batch
            delete_query = delete(PoseDetection).where(PoseDetection.id.in_(ids_to_delete))
            await session.execute(delete_query)
            await session.commit()
            
            batch_size = len(ids_to_delete)
            cleaned_count += batch_size
            
            logger.debug(f"Deleted {batch_size} pose detection records (total: {cleaned_count})")
            
            # Small delay to avoid overwhelming the database
            await asyncio.sleep(0.1)
        
        return {
            "cleaned_count": cleaned_count,
            "retention_days": self.retention_days,
            "cutoff_date": cutoff_date.isoformat()
        }


class OldMetricsCleanup(CleanupTask):
    """Cleanup old system metrics."""
    
    def __init__(self, settings: Settings):
        super().__init__("old_metrics_cleanup", settings)
        self.retention_days = settings.metrics_retention_days
        self.batch_size = settings.cleanup_batch_size
    
    async def execute(self, session: AsyncSession) -> Dict[str, Any]:
        """Execute metrics cleanup."""
        if self.retention_days <= 0:
            return {"cleaned_count": 0, "message": "Metrics retention disabled"}
        
        cutoff_date = datetime.utcnow() - timedelta(days=self.retention_days)
        
        # Count records to be deleted
        count_query = select(func.count(SystemMetric.id)).where(
            SystemMetric.created_at < cutoff_date
        )
        total_count = await session.scalar(count_query)
        
        if total_count == 0:
            return {"cleaned_count": 0, "message": "No old metrics to clean"}
        
        # Delete in batches
        cleaned_count = 0
        while cleaned_count < total_count:
            # Get batch of IDs to delete
            id_query = select(SystemMetric.id).where(
                SystemMetric.created_at < cutoff_date
            ).limit(self.batch_size)
            
            result = await session.execute(id_query)
            ids_to_delete = [row[0] for row in result.fetchall()]
            
            if not ids_to_delete:
                break
            
            # Delete batch
            delete_query = delete(SystemMetric).where(SystemMetric.id.in_(ids_to_delete))
            await session.execute(delete_query)
            await session.commit()
            
            batch_size = len(ids_to_delete)
            cleaned_count += batch_size
            
            logger.debug(f"Deleted {batch_size} metric records (total: {cleaned_count})")
            
            # Small delay to avoid overwhelming the database
            await asyncio.sleep(0.1)
        
        return {
            "cleaned_count": cleaned_count,
            "retention_days": self.retention_days,
            "cutoff_date": cutoff_date.isoformat()
        }


class OldAuditLogCleanup(CleanupTask):
    """Cleanup old audit logs."""
    
    def __init__(self, settings: Settings):
        super().__init__("old_audit_log_cleanup", settings)
        self.retention_days = settings.audit_log_retention_days
        self.batch_size = settings.cleanup_batch_size
    
    async def execute(self, session: AsyncSession) -> Dict[str, Any]:
        """Execute audit log cleanup."""
        if self.retention_days <= 0:
            return {"cleaned_count": 0, "message": "Audit log retention disabled"}
        
        cutoff_date = datetime.utcnow() - timedelta(days=self.retention_days)
        
        # Count records to be deleted
        count_query = select(func.count(AuditLog.id)).where(
            AuditLog.created_at < cutoff_date
        )
        total_count = await session.scalar(count_query)
        
        if total_count == 0:
            return {"cleaned_count": 0, "message": "No old audit logs to clean"}
        
        # Delete in batches
        cleaned_count = 0
        while cleaned_count < total_count:
            # Get batch of IDs to delete
            id_query = select(AuditLog.id).where(
                AuditLog.created_at < cutoff_date
            ).limit(self.batch_size)
            
            result = await session.execute(id_query)
            ids_to_delete = [row[0] for row in result.fetchall()]
            
            if not ids_to_delete:
                break
            
            # Delete batch
            delete_query = delete(AuditLog).where(AuditLog.id.in_(ids_to_delete))
            await session.execute(delete_query)
            await session.commit()
            
            batch_size = len(ids_to_delete)
            cleaned_count += batch_size
            
            logger.debug(f"Deleted {batch_size} audit log records (total: {cleaned_count})")
            
            # Small delay to avoid overwhelming the database
            await asyncio.sleep(0.1)
        
        return {
            "cleaned_count": cleaned_count,
            "retention_days": self.retention_days,
            "cutoff_date": cutoff_date.isoformat()
        }


class OrphanedSessionCleanup(CleanupTask):
    """Cleanup orphaned sessions (sessions without associated data)."""
    
    def __init__(self, settings: Settings):
        super().__init__("orphaned_session_cleanup", settings)
        self.orphan_threshold_days = settings.orphaned_session_threshold_days
        self.batch_size = settings.cleanup_batch_size
    
    async def execute(self, session: AsyncSession) -> Dict[str, Any]:
        """Execute orphaned session cleanup."""
        if self.orphan_threshold_days <= 0:
            return {"cleaned_count": 0, "message": "Orphaned session cleanup disabled"}
        
        cutoff_date = datetime.utcnow() - timedelta(days=self.orphan_threshold_days)
        
        # Find sessions that are old and have no associated CSI data or pose detections
        orphaned_sessions_query = select(Session.id).where(
            and_(
                Session.created_at < cutoff_date,
                Session.status.in_(["completed", "failed", "cancelled"]),
                ~Session.id.in_(select(CSIData.session_id).where(CSIData.session_id.isnot(None))),
                ~Session.id.in_(select(PoseDetection.session_id))
            )
        )
        
        result = await session.execute(orphaned_sessions_query)
        orphaned_ids = [row[0] for row in result.fetchall()]
        
        if not orphaned_ids:
            return {"cleaned_count": 0, "message": "No orphaned sessions to clean"}
        
        # Delete orphaned sessions
        delete_query = delete(Session).where(Session.id.in_(orphaned_ids))
        await session.execute(delete_query)
        await session.commit()
        
        cleaned_count = len(orphaned_ids)
        
        return {
            "cleaned_count": cleaned_count,
            "orphan_threshold_days": self.orphan_threshold_days,
            "cutoff_date": cutoff_date.isoformat()
        }


class InvalidDataCleanup(CleanupTask):
    """Cleanup invalid or corrupted data records."""
    
    def __init__(self, settings: Settings):
        super().__init__("invalid_data_cleanup", settings)
        self.batch_size = settings.cleanup_batch_size
    
    async def execute(self, session: AsyncSession) -> Dict[str, Any]:
        """Execute invalid data cleanup."""
        total_cleaned = 0
        
        # Clean invalid CSI data
        invalid_csi_query = select(CSIData.id).where(
            or_(
                CSIData.is_valid == False,
                CSIData.amplitude == None,
                CSIData.phase == None,
                CSIData.frequency <= 0,
                CSIData.bandwidth <= 0,
                CSIData.num_subcarriers <= 0
            )
        )
        
        result = await session.execute(invalid_csi_query)
        invalid_csi_ids = [row[0] for row in result.fetchall()]
        
        if invalid_csi_ids:
            delete_query = delete(CSIData).where(CSIData.id.in_(invalid_csi_ids))
            await session.execute(delete_query)
            total_cleaned += len(invalid_csi_ids)
            logger.debug(f"Deleted {len(invalid_csi_ids)} invalid CSI data records")
        
        # Clean invalid pose detections
        invalid_pose_query = select(PoseDetection.id).where(
            or_(
                PoseDetection.is_valid == False,
                PoseDetection.person_count < 0,
                and_(
                    PoseDetection.detection_confidence.isnot(None),
                    or_(
                        PoseDetection.detection_confidence < 0,
                        PoseDetection.detection_confidence > 1
                    )
                )
            )
        )
        
        result = await session.execute(invalid_pose_query)
        invalid_pose_ids = [row[0] for row in result.fetchall()]
        
        if invalid_pose_ids:
            delete_query = delete(PoseDetection).where(PoseDetection.id.in_(invalid_pose_ids))
            await session.execute(delete_query)
            total_cleaned += len(invalid_pose_ids)
            logger.debug(f"Deleted {len(invalid_pose_ids)} invalid pose detection records")
        
        await session.commit()
        
        return {
            "cleaned_count": total_cleaned,
            "invalid_csi_count": len(invalid_csi_ids) if invalid_csi_ids else 0,
            "invalid_pose_count": len(invalid_pose_ids) if invalid_pose_ids else 0,
        }


class CleanupManager:
    """Manager for all cleanup tasks."""
    
    def __init__(self, settings: Settings):
        self.settings = settings
        self.db_manager = get_database_manager(settings)
        self.tasks = self._initialize_tasks()
        self.running = False
        self.last_run = None
        self.run_count = 0
        self.total_cleaned = 0
    
    def _initialize_tasks(self) -> List[CleanupTask]:
        """Initialize all cleanup tasks."""
        tasks = [
            OldCSIDataCleanup(self.settings),
            OldPoseDetectionCleanup(self.settings),
            OldMetricsCleanup(self.settings),
            OldAuditLogCleanup(self.settings),
            OrphanedSessionCleanup(self.settings),
            InvalidDataCleanup(self.settings),
        ]
        
        # Filter enabled tasks
        enabled_tasks = [task for task in tasks if task.enabled]
        
        logger.info(f"Initialized {len(enabled_tasks)} cleanup tasks")
        return enabled_tasks
    
    async def run_all_tasks(self) -> Dict[str, Any]:
        """Run all cleanup tasks."""
        if self.running:
            return {"status": "already_running", "message": "Cleanup already in progress"}
        
        self.running = True
        start_time = datetime.utcnow()
        
        try:
            logger.info("Starting cleanup tasks")
            
            results = []
            total_cleaned = 0
            
            async with self.db_manager.get_async_session() as session:
                for task in self.tasks:
                    if not task.enabled:
                        continue
                    
                    result = await task.run(session)
                    results.append(result)
                    total_cleaned += result.get("cleaned_count", 0)
            
            self.last_run = start_time
            self.run_count += 1
            self.total_cleaned += total_cleaned
            
            duration = (datetime.utcnow() - start_time).total_seconds()
            
            logger.info(
                f"Cleanup tasks completed: cleaned {total_cleaned} items "
                f"in {duration:.2f} seconds"
            )
            
            return {
                "status": "completed",
                "start_time": start_time.isoformat(),
                "duration_seconds": duration,
                "total_cleaned": total_cleaned,
                "task_results": results,
            }
            
        except Exception as e:
            logger.error(f"Cleanup tasks failed: {e}", exc_info=True)
            return {
                "status": "error",
                "start_time": start_time.isoformat(),
                "duration_seconds": (datetime.utcnow() - start_time).total_seconds(),
                "error": str(e),
                "total_cleaned": 0,
            }
        
        finally:
            self.running = False
    
    async def run_task(self, task_name: str) -> Dict[str, Any]:
        """Run a specific cleanup task."""
        task = next((t for t in self.tasks if t.name == task_name), None)
        
        if not task:
            return {
                "status": "error",
                "error": f"Task '{task_name}' not found",
                "available_tasks": [t.name for t in self.tasks]
            }
        
        if not task.enabled:
            return {
                "status": "error",
                "error": f"Task '{task_name}' is disabled"
            }
        
        async with self.db_manager.get_async_session() as session:
            return await task.run(session)
    
    def get_stats(self) -> Dict[str, Any]:
        """Get cleanup manager statistics."""
        return {
            "manager": {
                "running": self.running,
                "last_run": self.last_run.isoformat() if self.last_run else None,
                "run_count": self.run_count,
                "total_cleaned": self.total_cleaned,
            },
            "tasks": [task.get_stats() for task in self.tasks],
        }
    
    def enable_task(self, task_name: str) -> bool:
        """Enable a specific task."""
        task = next((t for t in self.tasks if t.name == task_name), None)
        if task:
            task.enabled = True
            return True
        return False
    
    def disable_task(self, task_name: str) -> bool:
        """Disable a specific task."""
        task = next((t for t in self.tasks if t.name == task_name), None)
        if task:
            task.enabled = False
            return True
        return False


# Global cleanup manager instance
_cleanup_manager: Optional[CleanupManager] = None


def get_cleanup_manager(settings: Settings) -> CleanupManager:
    """Get cleanup manager instance."""
    global _cleanup_manager
    if _cleanup_manager is None:
        _cleanup_manager = CleanupManager(settings)
    return _cleanup_manager


async def run_periodic_cleanup(settings: Settings):
    """Run periodic cleanup tasks."""
    cleanup_manager = get_cleanup_manager(settings)
    
    while True:
        try:
            await cleanup_manager.run_all_tasks()
            
            # Wait for next cleanup interval
            await asyncio.sleep(settings.cleanup_interval_seconds)
            
        except asyncio.CancelledError:
            logger.info("Periodic cleanup cancelled")
            break
        except Exception as e:
            logger.error(f"Periodic cleanup error: {e}", exc_info=True)
            # Wait before retrying
            await asyncio.sleep(60)