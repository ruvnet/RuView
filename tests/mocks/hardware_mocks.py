"""
Hardware simulation mocks for testing.

Provides realistic hardware behavior simulation for routers and sensors.
"""

import asyncio
import numpy as np
from datetime import datetime, timedelta
from typing import Dict, Any, List, Optional, Callable, AsyncGenerator
from unittest.mock import AsyncMock, MagicMock
import json
import random
from dataclasses import dataclass, field
from enum import Enum


class RouterStatus(Enum):
    """Router status enumeration."""
    OFFLINE = "offline"
    CONNECTING = "connecting"
    ONLINE = "online"
    ERROR = "error"
    MAINTENANCE = "maintenance"


class SignalQuality(Enum):
    """Signal quality levels."""
    POOR = "poor"
    FAIR = "fair"
    GOOD = "good"
    EXCELLENT = "excellent"


@dataclass
class RouterConfig:
    """Router configuration."""
    router_id: str
    frequency: float = 5.8e9  # 5.8 GHz
    bandwidth: float = 80e6   # 80 MHz
    num_antennas: int = 4
    num_subcarriers: int = 64
    tx_power: float = 20.0    # dBm
    location: Dict[str, float] = field(default_factory=lambda: {"x": 0, "y": 0, "z": 0})
    firmware_version: str = "1.2.3"


class MockWiFiRouter:
    """Mock WiFi router with CSI capabilities."""
    
    def __init__(self, config: RouterConfig):
        self.config = config
        self.status = RouterStatus.OFFLINE
        self.signal_quality = SignalQuality.GOOD
        self.is_streaming = False
        self.connected_devices = []
        self.csi_data_buffer = []
        self.error_rate = 0.01  # 1% error rate
        self.latency_ms = 5.0
        self.throughput_mbps = 100.0
        self.temperature_celsius = 45.0
        self.uptime_seconds = 0
        self.last_heartbeat = None
        self.callbacks = {
            "on_status_change": [],
            "on_csi_data": [],
            "on_error": []
        }
        self._streaming_task = None
        self._heartbeat_task = None
    
    async def connect(self) -> bool:
        """Connect to router."""
        if self.status != RouterStatus.OFFLINE:
            return False
        
        self.status = RouterStatus.CONNECTING
        await self._notify_status_change()
        
        # Simulate connection delay
        await asyncio.sleep(0.1)
        
        # Simulate occasional connection failures
        if random.random() < 0.05:  # 5% failure rate
            self.status = RouterStatus.ERROR
            await self._notify_error("Connection failed")
            return False
        
        self.status = RouterStatus.ONLINE
        self.last_heartbeat = datetime.utcnow()
        await self._notify_status_change()
        
        # Start heartbeat
        self._heartbeat_task = asyncio.create_task(self._heartbeat_loop())
        
        return True
    
    async def disconnect(self):
        """Disconnect from router."""
        if self.status == RouterStatus.OFFLINE:
            return
        
        # Stop streaming if active
        if self.is_streaming:
            await self.stop_csi_streaming()
        
        # Stop heartbeat
        if self._heartbeat_task:
            self._heartbeat_task.cancel()
            try:
                await self._heartbeat_task
            except asyncio.CancelledError:
                pass
        
        self.status = RouterStatus.OFFLINE
        await self._notify_status_change()
    
    async def start_csi_streaming(self, sample_rate: int = 1000) -> bool:
        """Start CSI data streaming."""
        if self.status != RouterStatus.ONLINE:
            return False
        
        if self.is_streaming:
            return False
        
        self.is_streaming = True
        self._streaming_task = asyncio.create_task(self._csi_streaming_loop(sample_rate))
        
        return True
    
    async def stop_csi_streaming(self):
        """Stop CSI data streaming."""
        if not self.is_streaming:
            return
        
        self.is_streaming = False
        
        if self._streaming_task:
            self._streaming_task.cancel()
            try:
                await self._streaming_task
            except asyncio.CancelledError:
                pass
    
    async def _csi_streaming_loop(self, sample_rate: int):
        """CSI data streaming loop."""
        interval = 1.0 / sample_rate
        
        try:
            while self.is_streaming:
                # Generate CSI data
                csi_data = self._generate_csi_sample()
                
                # Add to buffer
                self.csi_data_buffer.append(csi_data)
                
                # Keep buffer size manageable
                if len(self.csi_data_buffer) > 1000:
                    self.csi_data_buffer = self.csi_data_buffer[-1000:]
                
                # Notify callbacks
                await self._notify_csi_data(csi_data)
                
                # Simulate processing delay and jitter
                actual_interval = interval * random.uniform(0.9, 1.1)
                await asyncio.sleep(actual_interval)
                
        except asyncio.CancelledError:
            pass
    
    async def _heartbeat_loop(self):
        """Heartbeat loop to maintain connection."""
        try:
            while self.status == RouterStatus.ONLINE:
                self.last_heartbeat = datetime.utcnow()
                self.uptime_seconds += 1
                
                # Simulate temperature variations
                self.temperature_celsius += random.uniform(-1, 1)
                self.temperature_celsius = max(30, min(80, self.temperature_celsius))
                
                # Check for overheating
                if self.temperature_celsius > 75:
                    self.signal_quality = SignalQuality.POOR
                    await self._notify_error("High temperature warning")
                
                await asyncio.sleep(1.0)
                
        except asyncio.CancelledError:
            pass
    
    def _generate_csi_sample(self) -> Dict[str, Any]:
        """Generate realistic CSI sample."""
        # Base amplitude and phase matrices
        amplitude = np.random.uniform(0.2, 0.8, (self.config.num_antennas, self.config.num_subcarriers))
        phase = np.random.uniform(-np.pi, np.pi, (self.config.num_antennas, self.config.num_subcarriers))
        
        # Add signal quality effects
        if self.signal_quality == SignalQuality.POOR:
            noise_level = 0.3
        elif self.signal_quality == SignalQuality.FAIR:
            noise_level = 0.2
        elif self.signal_quality == SignalQuality.GOOD:
            noise_level = 0.1
        else:  # EXCELLENT
            noise_level = 0.05
        
        # Add noise
        amplitude += np.random.normal(0, noise_level, amplitude.shape)
        phase += np.random.normal(0, noise_level * np.pi, phase.shape)
        
        # Clip values
        amplitude = np.clip(amplitude, 0, 1)
        phase = np.mod(phase + np.pi, 2 * np.pi) - np.pi
        
        # Simulate packet errors
        if random.random() < self.error_rate:
            # Corrupt some data
            corruption_mask = np.random.random(amplitude.shape) < 0.1
            amplitude[corruption_mask] = 0
            phase[corruption_mask] = 0
        
        return {
            "timestamp": datetime.utcnow().isoformat(),
            "router_id": self.config.router_id,
            "amplitude": amplitude.tolist(),
            "phase": phase.tolist(),
            "frequency": self.config.frequency,
            "bandwidth": self.config.bandwidth,
            "num_antennas": self.config.num_antennas,
            "num_subcarriers": self.config.num_subcarriers,
            "signal_quality": self.signal_quality.value,
            "temperature": self.temperature_celsius,
            "tx_power": self.config.tx_power,
            "sequence_number": len(self.csi_data_buffer)
        }
    
    def register_callback(self, event: str, callback: Callable):
        """Register event callback."""
        if event in self.callbacks:
            self.callbacks[event].append(callback)
    
    def unregister_callback(self, event: str, callback: Callable):
        """Unregister event callback."""
        if event in self.callbacks and callback in self.callbacks[event]:
            self.callbacks[event].remove(callback)
    
    async def _notify_status_change(self):
        """Notify status change callbacks."""
        for callback in self.callbacks["on_status_change"]:
            try:
                if asyncio.iscoroutinefunction(callback):
                    await callback(self.status)
                else:
                    callback(self.status)
            except Exception:
                pass  # Ignore callback errors
    
    async def _notify_csi_data(self, data: Dict[str, Any]):
        """Notify CSI data callbacks."""
        for callback in self.callbacks["on_csi_data"]:
            try:
                if asyncio.iscoroutinefunction(callback):
                    await callback(data)
                else:
                    callback(data)
            except Exception:
                pass
    
    async def _notify_error(self, error_message: str):
        """Notify error callbacks."""
        for callback in self.callbacks["on_error"]:
            try:
                if asyncio.iscoroutinefunction(callback):
                    await callback(error_message)
                else:
                    callback(error_message)
            except Exception:
                pass
    
    def get_status(self) -> Dict[str, Any]:
        """Get router status information."""
        return {
            "router_id": self.config.router_id,
            "status": self.status.value,
            "signal_quality": self.signal_quality.value,
            "is_streaming": self.is_streaming,
            "connected_devices": len(self.connected_devices),
            "temperature": self.temperature_celsius,
            "uptime_seconds": self.uptime_seconds,
            "last_heartbeat": self.last_heartbeat.isoformat() if self.last_heartbeat else None,
            "error_rate": self.error_rate,
            "latency_ms": self.latency_ms,
            "throughput_mbps": self.throughput_mbps,
            "firmware_version": self.config.firmware_version,
            "location": self.config.location
        }
    
    def set_signal_quality(self, quality: SignalQuality):
        """Set signal quality for testing."""
        self.signal_quality = quality
    
    def set_error_rate(self, error_rate: float):
        """Set error rate for testing."""
        self.error_rate = max(0, min(1, error_rate))
    
    def simulate_interference(self, duration_seconds: float = 5.0):
        """Simulate interference for testing."""
        async def interference_task():
            original_quality = self.signal_quality
            self.signal_quality = SignalQuality.POOR
            await asyncio.sleep(duration_seconds)
            self.signal_quality = original_quality
        
        asyncio.create_task(interference_task())
    
    def get_csi_buffer(self) -> List[Dict[str, Any]]:
        """Get CSI data buffer."""
        return self.csi_data_buffer.copy()
    
    def clear_csi_buffer(self):
        """Clear CSI data buffer."""
        self.csi_data_buffer.clear()


class MockRouterNetwork:
    """Mock network of WiFi routers."""
    
    def __init__(self):
        self.routers = {}
        self.network_topology = {}
        self.interference_sources = []
        self.global_callbacks = {
            "on_router_added": [],
            "on_router_removed": [],
            "on_network_event": []
        }
    
    def add_router(self, config: RouterConfig) -> MockWiFiRouter:
        """Add router to network."""
        if config.router_id in self.routers:
            raise ValueError(f"Router {config.router_id} already exists")
        
        router = MockWiFiRouter(config)
        self.routers[config.router_id] = router
        
        # Register for router events
        router.register_callback("on_status_change", self._on_router_status_change)
        router.register_callback("on_error", self._on_router_error)
        
        # Notify callbacks
        for callback in self.global_callbacks["on_router_added"]:
            callback(router)
        
        return router
    
    def remove_router(self, router_id: str) -> bool:
        """Remove router from network."""
        if router_id not in self.routers:
            return False
        
        router = self.routers[router_id]
        
        # Disconnect if connected
        if router.status != RouterStatus.OFFLINE:
            asyncio.create_task(router.disconnect())
        
        del self.routers[router_id]
        
        # Notify callbacks
        for callback in self.global_callbacks["on_router_removed"]:
            callback(router_id)
        
        return True
    
    def get_router(self, router_id: str) -> Optional[MockWiFiRouter]:
        """Get router by ID."""
        return self.routers.get(router_id)
    
    def get_all_routers(self) -> Dict[str, MockWiFiRouter]:
        """Get all routers."""
        return self.routers.copy()
    
    async def connect_all_routers(self) -> Dict[str, bool]:
        """Connect all routers."""
        results = {}
        tasks = []
        
        for router_id, router in self.routers.items():
            task = asyncio.create_task(router.connect())
            tasks.append((router_id, task))
        
        for router_id, task in tasks:
            try:
                result = await task
                results[router_id] = result
            except Exception:
                results[router_id] = False
        
        return results
    
    async def disconnect_all_routers(self):
        """Disconnect all routers."""
        tasks = []
        
        for router in self.routers.values():
            if router.status != RouterStatus.OFFLINE:
                task = asyncio.create_task(router.disconnect())
                tasks.append(task)
        
        if tasks:
            await asyncio.gather(*tasks, return_exceptions=True)
    
    async def start_all_streaming(self, sample_rate: int = 1000) -> Dict[str, bool]:
        """Start CSI streaming on all routers."""
        results = {}
        
        for router_id, router in self.routers.items():
            if router.status == RouterStatus.ONLINE:
                result = await router.start_csi_streaming(sample_rate)
                results[router_id] = result
            else:
                results[router_id] = False
        
        return results
    
    async def stop_all_streaming(self):
        """Stop CSI streaming on all routers."""
        tasks = []
        
        for router in self.routers.values():
            if router.is_streaming:
                task = asyncio.create_task(router.stop_csi_streaming())
                tasks.append(task)
        
        if tasks:
            await asyncio.gather(*tasks, return_exceptions=True)
    
    def get_network_status(self) -> Dict[str, Any]:
        """Get overall network status."""
        total_routers = len(self.routers)
        online_routers = sum(1 for r in self.routers.values() if r.status == RouterStatus.ONLINE)
        streaming_routers = sum(1 for r in self.routers.values() if r.is_streaming)
        
        return {
            "total_routers": total_routers,
            "online_routers": online_routers,
            "streaming_routers": streaming_routers,
            "network_health": online_routers / max(total_routers, 1),
            "interference_sources": len(self.interference_sources),
            "timestamp": datetime.utcnow().isoformat()
        }
    
    def simulate_network_partition(self, router_ids: List[str], duration_seconds: float = 10.0):
        """Simulate network partition for testing."""
        async def partition_task():
            # Disconnect specified routers
            affected_routers = [self.routers[rid] for rid in router_ids if rid in self.routers]
            
            for router in affected_routers:
                if router.status == RouterStatus.ONLINE:
                    router.status = RouterStatus.ERROR
                    await router._notify_status_change()
            
            await asyncio.sleep(duration_seconds)
            
            # Reconnect routers
            for router in affected_routers:
                if router.status == RouterStatus.ERROR:
                    await router.connect()
        
        asyncio.create_task(partition_task())
    
    def add_interference_source(self, location: Dict[str, float], strength: float, frequency: float):
        """Add interference source."""
        interference = {
            "id": f"interference_{len(self.interference_sources)}",
            "location": location,
            "strength": strength,
            "frequency": frequency,
            "active": True
        }
        
        self.interference_sources.append(interference)
        
        # Affect nearby routers
        for router in self.routers.values():
            distance = self._calculate_distance(router.config.location, location)
            if distance < 50:  # Within 50 meters
                if strength > 0.5:
                    router.set_signal_quality(SignalQuality.POOR)
                elif strength > 0.3:
                    router.set_signal_quality(SignalQuality.FAIR)
    
    def _calculate_distance(self, loc1: Dict[str, float], loc2: Dict[str, float]) -> float:
        """Calculate distance between two locations."""
        dx = loc1.get("x", 0) - loc2.get("x", 0)
        dy = loc1.get("y", 0) - loc2.get("y", 0)
        dz = loc1.get("z", 0) - loc2.get("z", 0)
        return np.sqrt(dx**2 + dy**2 + dz**2)
    
    async def _on_router_status_change(self, status: RouterStatus):
        """Handle router status change."""
        for callback in self.global_callbacks["on_network_event"]:
            await callback("router_status_change", {"status": status})
    
    async def _on_router_error(self, error_message: str):
        """Handle router error."""
        for callback in self.global_callbacks["on_network_event"]:
            await callback("router_error", {"error": error_message})
    
    def register_global_callback(self, event: str, callback: Callable):
        """Register global network callback."""
        if event in self.global_callbacks:
            self.global_callbacks[event].append(callback)


class MockSensorArray:
    """Mock sensor array for environmental monitoring."""
    
    def __init__(self, sensor_id: str, location: Dict[str, float]):
        self.sensor_id = sensor_id
        self.location = location
        self.is_active = False
        self.sensors = {
            "temperature": {"value": 22.0, "unit": "celsius", "range": (15, 35)},
            "humidity": {"value": 45.0, "unit": "percent", "range": (30, 70)},
            "pressure": {"value": 1013.25, "unit": "hPa", "range": (980, 1050)},
            "light": {"value": 300.0, "unit": "lux", "range": (0, 1000)},
            "motion": {"value": False, "unit": "boolean", "range": (False, True)},
            "sound": {"value": 35.0, "unit": "dB", "range": (20, 80)}
        }
        self.reading_history = []
        self.callbacks = []
    
    async def start_monitoring(self, interval_seconds: float = 1.0):
        """Start sensor monitoring."""
        if self.is_active:
            return False
        
        self.is_active = True
        asyncio.create_task(self._monitoring_loop(interval_seconds))
        return True
    
    def stop_monitoring(self):
        """Stop sensor monitoring."""
        self.is_active = False
    
    async def _monitoring_loop(self, interval: float):
        """Sensor monitoring loop."""
        try:
            while self.is_active:
                reading = self._generate_sensor_reading()
                self.reading_history.append(reading)
                
                # Keep history manageable
                if len(self.reading_history) > 1000:
                    self.reading_history = self.reading_history[-1000:]
                
                # Notify callbacks
                for callback in self.callbacks:
                    try:
                        if asyncio.iscoroutinefunction(callback):
                            await callback(reading)
                        else:
                            callback(reading)
                    except Exception:
                        pass
                
                await asyncio.sleep(interval)
                
        except asyncio.CancelledError:
            pass
    
    def _generate_sensor_reading(self) -> Dict[str, Any]:
        """Generate realistic sensor reading."""
        reading = {
            "sensor_id": self.sensor_id,
            "timestamp": datetime.utcnow().isoformat(),
            "location": self.location,
            "readings": {}
        }
        
        for sensor_name, config in self.sensors.items():
            if sensor_name == "motion":
                # Motion detection with some randomness
                reading["readings"][sensor_name] = random.random() < 0.1  # 10% chance of motion
            else:
                # Continuous sensors with drift
                current_value = config["value"]
                min_val, max_val = config["range"]
                
                # Add small random drift
                drift = random.uniform(-0.1, 0.1) * (max_val - min_val)
                new_value = current_value + drift
                
                # Keep within range
                new_value = max(min_val, min(max_val, new_value))
                
                config["value"] = new_value
                reading["readings"][sensor_name] = {
                    "value": round(new_value, 2),
                    "unit": config["unit"]
                }
        
        return reading
    
    def register_callback(self, callback: Callable):
        """Register sensor callback."""
        self.callbacks.append(callback)
    
    def unregister_callback(self, callback: Callable):
        """Unregister sensor callback."""
        if callback in self.callbacks:
            self.callbacks.remove(callback)
    
    def get_latest_reading(self) -> Optional[Dict[str, Any]]:
        """Get latest sensor reading."""
        return self.reading_history[-1] if self.reading_history else None
    
    def get_reading_history(self, limit: int = 100) -> List[Dict[str, Any]]:
        """Get sensor reading history."""
        return self.reading_history[-limit:]
    
    def simulate_event(self, event_type: str, duration_seconds: float = 5.0):
        """Simulate environmental event."""
        async def event_task():
            if event_type == "motion_detected":
                self.sensors["motion"]["value"] = True
                await asyncio.sleep(duration_seconds)
                self.sensors["motion"]["value"] = False
            
            elif event_type == "temperature_spike":
                original_temp = self.sensors["temperature"]["value"]
                self.sensors["temperature"]["value"] = min(35, original_temp + 10)
                await asyncio.sleep(duration_seconds)
                self.sensors["temperature"]["value"] = original_temp
            
            elif event_type == "loud_noise":
                original_sound = self.sensors["sound"]["value"]
                self.sensors["sound"]["value"] = min(80, original_sound + 20)
                await asyncio.sleep(duration_seconds)
                self.sensors["sound"]["value"] = original_sound
        
        asyncio.create_task(event_task())


# Utility functions for creating test hardware setups
def create_test_router_network(num_routers: int = 3) -> MockRouterNetwork:
    """Create test router network."""
    network = MockRouterNetwork()
    
    for i in range(num_routers):
        config = RouterConfig(
            router_id=f"router_{i:03d}",
            location={"x": i * 10, "y": 0, "z": 2.5}
        )
        network.add_router(config)
    
    return network


def create_test_sensor_array(num_sensors: int = 2) -> List[MockSensorArray]:
    """Create test sensor array."""
    sensors = []
    
    for i in range(num_sensors):
        sensor = MockSensorArray(
            sensor_id=f"sensor_{i:03d}",
            location={"x": i * 5, "y": 5, "z": 1.0}
        )
        sensors.append(sensor)
    
    return sensors


async def setup_test_hardware_environment() -> Dict[str, Any]:
    """Setup complete test hardware environment."""
    # Create router network
    router_network = create_test_router_network(3)
    
    # Create sensor arrays
    sensor_arrays = create_test_sensor_array(2)
    
    # Connect all routers
    router_results = await router_network.connect_all_routers()
    
    # Start sensor monitoring
    sensor_tasks = []
    for sensor in sensor_arrays:
        task = asyncio.create_task(sensor.start_monitoring(1.0))
        sensor_tasks.append(task)
    
    sensor_results = await asyncio.gather(*sensor_tasks)
    
    return {
        "router_network": router_network,
        "sensor_arrays": sensor_arrays,
        "router_connection_results": router_results,
        "sensor_start_results": sensor_results,
        "setup_timestamp": datetime.utcnow().isoformat()
    }


async def teardown_test_hardware_environment(environment: Dict[str, Any]):
    """Teardown test hardware environment."""
    # Stop sensor monitoring
    for sensor in environment["sensor_arrays"]:
        sensor.stop_monitoring()
    
    # Disconnect all routers
    await environment["router_network"].disconnect_all_routers()