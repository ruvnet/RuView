"""
WiFi-DensePose — WiFi-based human pose estimation using CSI data.

Usage:
    from wifi_densepose import WiFiDensePose

    system = WiFiDensePose()
    system.start()
    poses = system.get_latest_poses()
    system.stop()
"""

__version__ = "1.2.0"

import sys
import os
import logging

logger = logging.getLogger(__name__)

# Allow importing the v1 src package when installed from the repo
_v1_src = os.path.join(os.path.dirname(os.path.dirname(__file__)), "v1")
if os.path.isdir(_v1_src) and _v1_src not in sys.path:
    sys.path.insert(0, _v1_src)


class WiFiDensePose:
    """High-level facade for the WiFi-DensePose sensing system.

    This is the primary entry point documented in the README Quick Start.
    It wraps the underlying ServiceOrchestrator and exposes a simple
    start / get_latest_poses / stop interface.
    """

    def __init__(self, host: str = "0.0.0.0", port: int = 3000, **kwargs):
        self.host = host
        self.port = port
        self._config = kwargs
        self._orchestrator = None
        self._server_task = None
        self._poses = []
        self._running = False

    # ------------------------------------------------------------------
    # Public API (matches README Quick Start)
    # ------------------------------------------------------------------

    def start(self):
        """Start the sensing system (blocking until ready)."""
        import asyncio

        loop = _get_or_create_event_loop()
        loop.run_until_complete(self._async_start())

    async def _async_start(self):
        try:
            from src.config.settings import get_settings
            from src.services.orchestrator import ServiceOrchestrator

            settings = get_settings()
            self._orchestrator = ServiceOrchestrator(settings)
            await self._orchestrator.initialize()
            await self._orchestrator.start()
            self._running = True
            logger.info("WiFiDensePose system started on %s:%s", self.host, self.port)
        except ImportError:
            raise ImportError(
                "Core dependencies not found. Make sure you installed "
                "from the repository root:\n"
                "  cd wifi-densepose && pip install -e .\n"
                "Or install the v1 package:\n"
                "  cd wifi-densepose/v1 && pip install -e ."
            )

    def stop(self):
        """Stop the sensing system."""
        import asyncio

        if self._orchestrator is not None:
            loop = _get_or_create_event_loop()
            loop.run_until_complete(self._orchestrator.shutdown())
            self._running = False
            logger.info("WiFiDensePose system stopped")

    def get_latest_poses(self):
        """Return the most recent list of detected pose dicts."""
        if self._orchestrator is None:
            return []
        try:
            import asyncio

            loop = _get_or_create_event_loop()
            return loop.run_until_complete(self._fetch_poses())
        except Exception:
            return []

    async def _fetch_poses(self):
        try:
            pose_svc = self._orchestrator.pose_service
            if pose_svc and hasattr(pose_svc, "get_latest"):
                return await pose_svc.get_latest()
        except Exception:
            pass
        return []

    # ------------------------------------------------------------------
    # Context-manager support
    # ------------------------------------------------------------------

    def __enter__(self):
        self.start()
        return self

    def __exit__(self, *exc):
        self.stop()

    # ------------------------------------------------------------------
    # Convenience re-exports
    # ------------------------------------------------------------------

    @staticmethod
    def version():
        return __version__


def _get_or_create_event_loop():
    import asyncio

    try:
        return asyncio.get_event_loop()
    except RuntimeError:
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        return loop


__all__ = ["WiFiDensePose", "__version__"]
