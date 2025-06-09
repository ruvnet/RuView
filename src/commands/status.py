"""
Status command implementation for WiFi-DensePose API
"""

import asyncio
import json
import psutil
import time
from datetime import datetime, timedelta
from pathlib import Path
from typing import Dict, Any, Optional

from src.config.settings import Settings
from src.logger import get_logger

logger = get_logger(__name__)


async def status_command(
    settings: Settings,
    output_format: str = "text",
    detailed: bool = False
) -> None:
    """Show the status of the WiFi-DensePose API server."""
    
    logger.debug("Gathering server status information...")
    
    try:
        # Collect status information
        status_data = await _collect_status_data(settings, detailed)
        
        # Output status
        if output_format == "json":
            print(json.dumps(status_data, indent=2, default=str))
        else:
            _print_text_status(status_data, detailed)
            
    except Exception as e:
        logger.error(f"Failed to get status: {e}")
        raise


async def _collect_status_data(settings: Settings, detailed: bool) -> Dict[str, Any]:
    """Collect comprehensive status data."""
    
    status_data = {
        "timestamp": datetime.utcnow().isoformat(),
        "server": await _get_server_status(settings),
        "system": _get_system_status(),
        "configuration": _get_configuration_status(settings),
    }
    
    if detailed:
        status_data.update({
            "database": await _get_database_status(settings),
            "background_tasks": await _get_background_tasks_status(settings),
            "resources": _get_resource_usage(),
            "health": await _get_health_status(settings),
        })
    
    return status_data


async def _get_server_status(settings: Settings) -> Dict[str, Any]:
    """Get server process status."""
    
    from src.commands.stop import get_server_status
    
    status = get_server_status(settings)
    
    server_info = {
        "running": status["running"],
        "pid": status["pid"],
        "pid_file": status["pid_file"],
        "pid_file_exists": status["pid_file_exists"],
    }
    
    if status["running"] and status["pid"]:
        try:
            # Get process information
            process = psutil.Process(status["pid"])
            
            server_info.update({
                "start_time": datetime.fromtimestamp(process.create_time()).isoformat(),
                "uptime_seconds": time.time() - process.create_time(),
                "memory_usage_mb": process.memory_info().rss / (1024 * 1024),
                "cpu_percent": process.cpu_percent(),
                "status": process.status(),
                "num_threads": process.num_threads(),
                "connections": len(process.connections()) if hasattr(process, 'connections') else None,
            })
            
        except (psutil.NoSuchProcess, psutil.AccessDenied) as e:
            server_info["error"] = f"Cannot access process info: {e}"
    
    return server_info


def _get_system_status() -> Dict[str, Any]:
    """Get system status information."""
    
    uname_info = psutil.os.uname()
    return {
        "hostname": uname_info.nodename,
        "platform": uname_info.sysname,
        "architecture": uname_info.machine,
        "python_version": f"{psutil.sys.version_info.major}.{psutil.sys.version_info.minor}.{psutil.sys.version_info.micro}",
        "boot_time": datetime.fromtimestamp(psutil.boot_time()).isoformat(),
        "uptime_seconds": time.time() - psutil.boot_time(),
    }


def _get_configuration_status(settings: Settings) -> Dict[str, Any]:
    """Get configuration status."""
    
    return {
        "environment": settings.environment,
        "debug": settings.debug,
        "version": settings.version,
        "host": settings.host,
        "port": settings.port,
        "database_configured": bool(settings.database_url or (settings.db_host and settings.db_name)),
        "redis_enabled": settings.redis_enabled,
        "monitoring_enabled": settings.monitoring_interval_seconds > 0,
        "cleanup_enabled": settings.cleanup_interval_seconds > 0,
        "backup_enabled": settings.backup_interval_seconds > 0,
    }


async def _get_database_status(settings: Settings) -> Dict[str, Any]:
    """Get database status."""
    
    db_status = {
        "connected": False,
        "connection_pool": None,
        "tables": {},
        "error": None,
    }
    
    try:
        from src.database.connection import get_database_manager
        
        db_manager = get_database_manager(settings)
        
        # Test connection
        await db_manager.test_connection()
        db_status["connected"] = True
        
        # Get connection stats
        connection_stats = await db_manager.get_connection_stats()
        db_status["connection_pool"] = connection_stats
        
        # Get table counts
        async with db_manager.get_async_session() as session:
            from sqlalchemy import text, func
            from src.database.models import Device, Session, CSIData, PoseDetection, SystemMetric, AuditLog
            
            tables = {
                "devices": Device,
                "sessions": Session,
                "csi_data": CSIData,
                "pose_detections": PoseDetection,
                "system_metrics": SystemMetric,
                "audit_logs": AuditLog,
            }
            
            for table_name, model in tables.items():
                try:
                    result = await session.execute(
                        text(f"SELECT COUNT(*) FROM {table_name}")
                    )
                    count = result.scalar()
                    db_status["tables"][table_name] = {"count": count}
                except Exception as e:
                    db_status["tables"][table_name] = {"error": str(e)}
        
    except Exception as e:
        db_status["error"] = str(e)
    
    return db_status


async def _get_background_tasks_status(settings: Settings) -> Dict[str, Any]:
    """Get background tasks status."""
    
    tasks_status = {}
    
    try:
        # Cleanup tasks
        from src.tasks.cleanup import get_cleanup_manager
        cleanup_manager = get_cleanup_manager(settings)
        tasks_status["cleanup"] = cleanup_manager.get_stats()
        
    except Exception as e:
        tasks_status["cleanup"] = {"error": str(e)}
    
    try:
        # Monitoring tasks
        from src.tasks.monitoring import get_monitoring_manager
        monitoring_manager = get_monitoring_manager(settings)
        tasks_status["monitoring"] = monitoring_manager.get_stats()
        
    except Exception as e:
        tasks_status["monitoring"] = {"error": str(e)}
    
    try:
        # Backup tasks
        from src.tasks.backup import get_backup_manager
        backup_manager = get_backup_manager(settings)
        tasks_status["backup"] = backup_manager.get_stats()
        
    except Exception as e:
        tasks_status["backup"] = {"error": str(e)}
    
    return tasks_status


def _get_resource_usage() -> Dict[str, Any]:
    """Get system resource usage."""
    
    # CPU usage
    cpu_percent = psutil.cpu_percent(interval=1)
    cpu_count = psutil.cpu_count()
    
    # Memory usage
    memory = psutil.virtual_memory()
    swap = psutil.swap_memory()
    
    # Disk usage
    disk = psutil.disk_usage('/')
    
    # Network I/O
    network = psutil.net_io_counters()
    
    return {
        "cpu": {
            "usage_percent": cpu_percent,
            "count": cpu_count,
        },
        "memory": {
            "total_mb": memory.total / (1024 * 1024),
            "used_mb": memory.used / (1024 * 1024),
            "available_mb": memory.available / (1024 * 1024),
            "usage_percent": memory.percent,
        },
        "swap": {
            "total_mb": swap.total / (1024 * 1024),
            "used_mb": swap.used / (1024 * 1024),
            "usage_percent": swap.percent,
        },
        "disk": {
            "total_gb": disk.total / (1024 * 1024 * 1024),
            "used_gb": disk.used / (1024 * 1024 * 1024),
            "free_gb": disk.free / (1024 * 1024 * 1024),
            "usage_percent": (disk.used / disk.total) * 100,
        },
        "network": {
            "bytes_sent": network.bytes_sent,
            "bytes_recv": network.bytes_recv,
            "packets_sent": network.packets_sent,
            "packets_recv": network.packets_recv,
        } if network else None,
    }


async def _get_health_status(settings: Settings) -> Dict[str, Any]:
    """Get overall health status."""
    
    health = {
        "status": "healthy",
        "checks": {},
        "issues": [],
    }
    
    # Check database health
    try:
        from src.database.connection import get_database_manager
        
        db_manager = get_database_manager(settings)
        await db_manager.test_connection()
        health["checks"]["database"] = "healthy"
        
    except Exception as e:
        health["checks"]["database"] = "unhealthy"
        health["issues"].append(f"Database connection failed: {e}")
        health["status"] = "unhealthy"
    
    # Check disk space
    disk = psutil.disk_usage('/')
    disk_usage_percent = (disk.used / disk.total) * 100
    
    if disk_usage_percent > 90:
        health["checks"]["disk_space"] = "critical"
        health["issues"].append(f"Disk usage critical: {disk_usage_percent:.1f}%")
        health["status"] = "critical"
    elif disk_usage_percent > 80:
        health["checks"]["disk_space"] = "warning"
        health["issues"].append(f"Disk usage high: {disk_usage_percent:.1f}%")
        if health["status"] == "healthy":
            health["status"] = "warning"
    else:
        health["checks"]["disk_space"] = "healthy"
    
    # Check memory usage
    memory = psutil.virtual_memory()
    
    if memory.percent > 90:
        health["checks"]["memory"] = "critical"
        health["issues"].append(f"Memory usage critical: {memory.percent:.1f}%")
        health["status"] = "critical"
    elif memory.percent > 80:
        health["checks"]["memory"] = "warning"
        health["issues"].append(f"Memory usage high: {memory.percent:.1f}%")
        if health["status"] == "healthy":
            health["status"] = "warning"
    else:
        health["checks"]["memory"] = "healthy"
    
    # Check log directory
    log_dir = Path(settings.log_directory)
    if log_dir.exists() and log_dir.is_dir():
        health["checks"]["log_directory"] = "healthy"
    else:
        health["checks"]["log_directory"] = "unhealthy"
        health["issues"].append(f"Log directory not accessible: {log_dir}")
        health["status"] = "unhealthy"
    
    # Check backup directory
    backup_dir = Path(settings.backup_directory)
    if backup_dir.exists() and backup_dir.is_dir():
        health["checks"]["backup_directory"] = "healthy"
    else:
        health["checks"]["backup_directory"] = "unhealthy"
        health["issues"].append(f"Backup directory not accessible: {backup_dir}")
        health["status"] = "unhealthy"
    
    return health


def _print_text_status(status_data: Dict[str, Any], detailed: bool) -> None:
    """Print status in human-readable text format."""
    
    print("=" * 60)
    print("WiFi-DensePose API Server Status")
    print("=" * 60)
    print(f"Timestamp: {status_data['timestamp']}")
    print()
    
    # Server status
    server = status_data["server"]
    print("🖥️  Server Status:")
    if server["running"]:
        print(f"   ✅ Running (PID: {server['pid']})")
        if "start_time" in server:
            uptime = timedelta(seconds=int(server["uptime_seconds"]))
            print(f"   ⏱️  Uptime: {uptime}")
            print(f"   💾 Memory: {server['memory_usage_mb']:.1f} MB")
            print(f"   🔧 CPU: {server['cpu_percent']:.1f}%")
            print(f"   🧵 Threads: {server['num_threads']}")
    else:
        print("   ❌ Not running")
        if server["pid_file_exists"]:
            print("   ⚠️  Stale PID file exists")
    print()
    
    # System status
    system = status_data["system"]
    print("🖥️  System:")
    print(f"   Hostname: {system['hostname']}")
    print(f"   Platform: {system['platform']} ({system['architecture']})")
    print(f"   Python: {system['python_version']}")
    uptime = timedelta(seconds=int(system["uptime_seconds"]))
    print(f"   Uptime: {uptime}")
    print()
    
    # Configuration
    config = status_data["configuration"]
    print("⚙️  Configuration:")
    print(f"   Environment: {config['environment']}")
    print(f"   Debug: {config['debug']}")
    print(f"   API Version: {config['version']}")
    print(f"   Listen: {config['host']}:{config['port']}")
    print(f"   Database: {'✅' if config['database_configured'] else '❌'}")
    print(f"   Redis: {'✅' if config['redis_enabled'] else '❌'}")
    print(f"   Monitoring: {'✅' if config['monitoring_enabled'] else '❌'}")
    print(f"   Cleanup: {'✅' if config['cleanup_enabled'] else '❌'}")
    print(f"   Backup: {'✅' if config['backup_enabled'] else '❌'}")
    print()
    
    if detailed:
        # Database status
        if "database" in status_data:
            db = status_data["database"]
            print("🗄️  Database:")
            if db["connected"]:
                print("   ✅ Connected")
                if "tables" in db:
                    print("   📊 Table counts:")
                    for table, info in db["tables"].items():
                        if "count" in info:
                            print(f"      {table}: {info['count']:,}")
                        else:
                            print(f"      {table}: Error - {info.get('error', 'Unknown')}")
            else:
                print(f"   ❌ Not connected: {db.get('error', 'Unknown error')}")
            print()
        
        # Background tasks
        if "background_tasks" in status_data:
            tasks = status_data["background_tasks"]
            print("🔄 Background Tasks:")
            for task_name, task_info in tasks.items():
                if "error" in task_info:
                    print(f"   ❌ {task_name}: {task_info['error']}")
                else:
                    manager_info = task_info.get("manager", {})
                    print(f"   📋 {task_name}:")
                    print(f"      Running: {manager_info.get('running', 'Unknown')}")
                    print(f"      Last run: {manager_info.get('last_run', 'Never')}")
                    print(f"      Run count: {manager_info.get('run_count', 0)}")
            print()
        
        # Resource usage
        if "resources" in status_data:
            resources = status_data["resources"]
            print("📊 Resource Usage:")
            
            cpu = resources["cpu"]
            print(f"   🔧 CPU: {cpu['usage_percent']:.1f}% ({cpu['count']} cores)")
            
            memory = resources["memory"]
            print(f"   💾 Memory: {memory['usage_percent']:.1f}% "
                  f"({memory['used_mb']:.0f}/{memory['total_mb']:.0f} MB)")
            
            disk = resources["disk"]
            print(f"   💿 Disk: {disk['usage_percent']:.1f}% "
                  f"({disk['used_gb']:.1f}/{disk['total_gb']:.1f} GB)")
            print()
        
        # Health status
        if "health" in status_data:
            health = status_data["health"]
            print("🏥 Health Status:")
            
            status_emoji = {
                "healthy": "✅",
                "warning": "⚠️",
                "critical": "❌",
                "unhealthy": "❌"
            }
            
            print(f"   Overall: {status_emoji.get(health['status'], '❓')} {health['status'].upper()}")
            
            if health["issues"]:
                print("   Issues:")
                for issue in health["issues"]:
                    print(f"      • {issue}")
            
            print("   Checks:")
            for check, status in health["checks"].items():
                emoji = status_emoji.get(status, "❓")
                print(f"      {emoji} {check}: {status}")
            print()
    
    print("=" * 60)


def get_quick_status(settings: Settings) -> str:
    """Get a quick one-line status."""
    
    from src.commands.stop import get_server_status
    
    status = get_server_status(settings)
    
    if status["running"]:
        return f"✅ Running (PID: {status['pid']})"
    elif status["pid_file_exists"]:
        return "⚠️  Not running (stale PID file)"
    else:
        return "❌ Not running"


async def check_health(settings: Settings) -> bool:
    """Quick health check - returns True if healthy."""
    
    try:
        status_data = await _collect_status_data(settings, detailed=True)
        
        # Check if server is running
        if not status_data["server"]["running"]:
            return False
        
        # Check health status
        if "health" in status_data:
            health_status = status_data["health"]["status"]
            return health_status in ["healthy", "warning"]
        
        return True
        
    except Exception:
        return False