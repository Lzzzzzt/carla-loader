#!/usr/bin/env python3
"""
CARLA Sensor Data Recorder

Records raw sensor data from CARLA to files for later replay in mock mode.

Usage:
    python record_carla.py --config config/config.json --output recordings/session_001 --duration 30

Output format:
    output_dir/
        manifest.json       # Session metadata
        sensors.jsonl       # Sensor packet metadata (one JSON per line)
        rgb_camera/         # Binary data files
            frame_000001.bin
        lidar/
            frame_000001.bin
"""

import argparse
import json
import os
import struct
import sys
import time
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from threading import Lock
from typing import Any

import carla
import numpy as np


@dataclass
class SensorInfo:
    """Sensor metadata"""
    sensor_id: str
    sensor_type: str
    carla_sensor: Any
    frame_count: int = 0
    output_dir: Path = field(default_factory=Path)


class CarlaRecorder:
    """Records CARLA sensor data to files"""
    
    def __init__(self, output_dir: str, config: dict):
        self.output_dir = Path(output_dir)
        self.config = config
        self.sensors: dict[str, SensorInfo] = {}
        self.jsonl_file = None
        self.jsonl_lock = Lock()
        self.start_time = None
        self.client = None
        self.world = None
        self.vehicle = None
        
    def connect(self):
        """Connect to CARLA server"""
        host = self.config["world"]["carla_host"]
        port = self.config["world"]["carla_port"]
        print(f"Connecting to CARLA at {host}:{port}...")
        
        self.client = carla.Client(host, port)
        self.client.set_timeout(10.0)
        self.world = self.client.get_world()
        print(f"Connected! Map: {self.world.get_map().name}")
        
    def setup_output(self):
        """Create output directory structure"""
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.jsonl_file = open(self.output_dir / "sensors.jsonl", "w")
        
    def spawn_vehicle(self):
        """Spawn ego vehicle"""
        vehicle_config = self.config["vehicles"][0]
        blueprint_library = self.world.get_blueprint_library()
        vehicle_bp = blueprint_library.find(vehicle_config["blueprint"])
        
        spawn_points = self.world.get_map().get_spawn_points()
        spawn_point = spawn_points[0] if spawn_points else carla.Transform()
        
        self.vehicle = self.world.spawn_actor(vehicle_bp, spawn_point)
        self.vehicle.set_autopilot(True)
        print(f"Spawned vehicle: {vehicle_config['blueprint']}")
        
    def spawn_sensors(self):
        """Spawn all sensors from config"""
        vehicle_config = self.config["vehicles"][0]
        blueprint_library = self.world.get_blueprint_library()
        
        for sensor_config in vehicle_config["sensors"]:
            sensor_id = sensor_config["id"]
            sensor_type = sensor_config["sensor_type"]
            
            # Get blueprint
            bp_name = self._get_blueprint_name(sensor_type)
            sensor_bp = blueprint_library.find(bp_name)
            
            # Set attributes
            for key, value in sensor_config.get("attributes", {}).items():
                if sensor_bp.has_attribute(key):
                    sensor_bp.set_attribute(key, str(value))
            
            # Transform
            transform_cfg = sensor_config["transform"]
            loc = transform_cfg["location"]
            rot = transform_cfg["rotation"]
            transform = carla.Transform(
                carla.Location(x=loc["x"], y=loc["y"], z=loc["z"]),
                carla.Rotation(pitch=rot["pitch"], yaw=rot["yaw"], roll=rot["roll"])
            )
            
            # Spawn and attach
            sensor = self.world.spawn_actor(sensor_bp, transform, attach_to=self.vehicle)
            
            # Create output directory for this sensor
            sensor_dir = self.output_dir / sensor_id
            sensor_dir.mkdir(exist_ok=True)
            
            self.sensors[sensor_id] = SensorInfo(
                sensor_id=sensor_id,
                sensor_type=sensor_type,
                carla_sensor=sensor,
                output_dir=sensor_dir
            )
            
            # Register callback
            sensor.listen(lambda data, sid=sensor_id: self._on_sensor_data(sid, data))
            print(f"Spawned sensor: {sensor_id} ({sensor_type})")
    
    def _get_blueprint_name(self, sensor_type: str) -> str:
        """Map sensor type to CARLA blueprint"""
        mapping = {
            "camera": "sensor.camera.rgb",
            "lidar": "sensor.lidar.ray_cast",
            "imu": "sensor.other.imu",
            "gnss": "sensor.other.gnss",
            "radar": "sensor.other.radar",
        }
        return mapping.get(sensor_type, sensor_type)
    
    def _on_sensor_data(self, sensor_id: str, data):
        """Handle sensor callback"""
        if self.start_time is None:
            return
            
        sensor_info = self.sensors[sensor_id]
        sensor_info.frame_count += 1
        frame_id = sensor_info.frame_count
        timestamp = data.timestamp
        
        # Prepare metadata
        metadata = {
            "sensor_id": sensor_id,
            "sensor_type": sensor_info.sensor_type,
            "timestamp": timestamp,
            "frame_id": frame_id,
        }
        
        # Handle different sensor types
        if sensor_info.sensor_type == "camera":
            metadata.update(self._handle_camera(sensor_info, frame_id, data))
        elif sensor_info.sensor_type == "lidar":
            metadata.update(self._handle_lidar(sensor_info, frame_id, data))
        elif sensor_info.sensor_type == "imu":
            metadata.update(self._handle_imu(data))
        elif sensor_info.sensor_type == "gnss":
            metadata.update(self._handle_gnss(data))
        elif sensor_info.sensor_type == "radar":
            metadata.update(self._handle_radar(sensor_info, frame_id, data))
        
        # Write metadata to JSONL
        with self.jsonl_lock:
            self.jsonl_file.write(json.dumps(metadata) + "\n")
            self.jsonl_file.flush()
    
    def _handle_camera(self, sensor_info: SensorInfo, frame_id: int, image) -> dict:
        """Handle camera data"""
        # Get raw bytes (BGRA format)
        array = np.frombuffer(image.raw_data, dtype=np.uint8)
        
        # Save to binary file
        filename = f"frame_{frame_id:06d}.bin"
        filepath = sensor_info.output_dir / filename
        array.tofile(filepath)
        
        return {
            "data_file": f"{sensor_info.sensor_id}/{filename}",
            "width": image.width,
            "height": image.height,
            "format": "bgra8",
        }
    
    def _handle_lidar(self, sensor_info: SensorInfo, frame_id: int, lidar_data) -> dict:
        """Handle LiDAR data"""
        # Each point is (x, y, z, intensity) as 4 floats = 16 bytes
        points = np.frombuffer(lidar_data.raw_data, dtype=np.float32)
        
        filename = f"frame_{frame_id:06d}.bin"
        filepath = sensor_info.output_dir / filename
        points.tofile(filepath)
        
        return {
            "data_file": f"{sensor_info.sensor_id}/{filename}",
            "num_points": len(points) // 4,
            "point_stride": 16,
        }
    
    def _handle_imu(self, imu_data) -> dict:
        """Handle IMU data - inline in JSONL"""
        return {
            "accelerometer": [
                imu_data.accelerometer.x,
                imu_data.accelerometer.y,
                imu_data.accelerometer.z,
            ],
            "gyroscope": [
                imu_data.gyroscope.x,
                imu_data.gyroscope.y,
                imu_data.gyroscope.z,
            ],
            "compass": imu_data.compass,
        }
    
    def _handle_gnss(self, gnss_data) -> dict:
        """Handle GNSS data - inline in JSONL"""
        return {
            "latitude": gnss_data.latitude,
            "longitude": gnss_data.longitude,
            "altitude": gnss_data.altitude,
        }
    
    def _handle_radar(self, sensor_info: SensorInfo, frame_id: int, radar_data) -> dict:
        """Handle Radar data"""
        # Each detection is (velocity, azimuth, altitude, depth) as 4 floats
        detections = np.frombuffer(radar_data.raw_data, dtype=np.float32)
        
        filename = f"frame_{frame_id:06d}.bin"
        filepath = sensor_info.output_dir / filename
        detections.tofile(filepath)
        
        return {
            "data_file": f"{sensor_info.sensor_id}/{filename}",
            "num_detections": len(radar_data),
        }
    
    def record(self, duration: float):
        """Start recording for specified duration"""
        print(f"\nRecording for {duration} seconds...")
        self.start_time = time.time()
        
        try:
            while time.time() - self.start_time < duration:
                # Small sleep to prevent busy loop
                time.sleep(0.01)
                
                # Progress
                elapsed = time.time() - self.start_time
                total_frames = sum(s.frame_count for s in self.sensors.values())
                print(f"\rElapsed: {elapsed:.1f}s / {duration}s | Frames: {total_frames}", end="")
        except KeyboardInterrupt:
            print("\nRecording stopped by user")
        
        print(f"\nRecording complete!")
    
    def save_manifest(self):
        """Save session manifest"""
        duration = time.time() - self.start_time if self.start_time else 0
        
        manifest = {
            "version": "1.0",
            "created_at": datetime.utcnow().isoformat() + "Z",
            "carla_version": self.client.get_server_version() if self.client else "unknown",
            "duration_sec": duration,
            "sensors": {
                sid: {
                    "sensor_type": info.sensor_type,
                    "frame_count": info.frame_count
                }
                for sid, info in self.sensors.items()
            }
        }
        
        with open(self.output_dir / "manifest.json", "w") as f:
            json.dump(manifest, f, indent=2)
        
        print(f"Manifest saved to {self.output_dir / 'manifest.json'}")
    
    def cleanup(self):
        """Destroy actors and close files"""
        print("Cleaning up...")
        
        # Stop sensors
        for sensor_info in self.sensors.values():
            sensor_info.carla_sensor.stop()
            sensor_info.carla_sensor.destroy()
        
        # Destroy vehicle
        if self.vehicle:
            self.vehicle.destroy()
        
        # Close JSONL file
        if self.jsonl_file:
            self.jsonl_file.close()
        
        print("Cleanup complete")


def main():
    parser = argparse.ArgumentParser(description="Record CARLA sensor data")
    parser.add_argument("--config", required=True, help="Path to config.json")
    parser.add_argument("--output", required=True, help="Output directory")
    parser.add_argument("--duration", type=float, default=30.0, help="Recording duration in seconds")
    args = parser.parse_args()
    
    # Load config
    with open(args.config) as f:
        config = json.load(f)
    
    recorder = CarlaRecorder(args.output, config)
    
    try:
        recorder.connect()
        recorder.setup_output()
        recorder.spawn_vehicle()
        recorder.spawn_sensors()
        recorder.record(args.duration)
        recorder.save_manifest()
    finally:
        recorder.cleanup()


if __name__ == "__main__":
    main()
