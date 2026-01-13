"""
Pose estimation API endpoints
"""

import logging
from typing import List, Optional, Dict, Any
from datetime import datetime, timedelta

from fastapi import APIRouter, Depends, HTTPException, Query, BackgroundTasks
from fastapi.responses import JSONResponse
from pydantic import BaseModel, Field

from src.api.dependencies import (
    get_pose_service,
    get_hardware_service,
    get_current_user,
    require_auth
)
from src.services.pose_service import PoseService
from src.services.hardware_service import HardwareService
from src.config.settings import get_settings

logger = logging.getLogger(__name__)
router = APIRouter()


# Request/Response models
class PoseEstimationRequest(BaseModel):
    """Request model for pose estimation."""
    
    zone_ids: Optional[List[str]] = Field(
        default=None,
        description="Specific zones to analyze (all zones if not specified)"
    )
    confidence_threshold: Optional[float] = Field(
        default=None,
        ge=0.0,
        le=1.0,
        description="Minimum confidence threshold for detections"
    )
    max_persons: Optional[int] = Field(
        default=None,
        ge=1,
        le=50,
        description="Maximum number of persons to detect"
    )
    include_keypoints: bool = Field(
        default=True,
        description="Include detailed keypoint data"
    )
    include_segmentation: bool = Field(
        default=False,
        description="Include DensePose segmentation masks"
    )


class PersonPose(BaseModel):
    """Person pose data model."""
    
    person_id: str = Field(..., description="Unique person identifier")
    confidence: float = Field(..., description="Detection confidence score")
    bounding_box: Dict[str, float] = Field(..., description="Person bounding box")
    keypoints: Optional[List[Dict[str, Any]]] = Field(
        default=None,
        description="Body keypoints with coordinates and confidence"
    )
    segmentation: Optional[Dict[str, Any]] = Field(
        default=None,
        description="DensePose segmentation data"
    )
    zone_id: Optional[str] = Field(
        default=None,
        description="Zone where person is detected"
    )
    activity: Optional[str] = Field(
        default=None,
        description="Detected activity"
    )
    timestamp: datetime = Field(..., description="Detection timestamp")


class PoseEstimationResponse(BaseModel):
    """Response model for pose estimation."""
    
    timestamp: datetime = Field(..., description="Analysis timestamp")
    frame_id: str = Field(..., description="Unique frame identifier")
    persons: List[PersonPose] = Field(..., description="Detected persons")
    zone_summary: Dict[str, int] = Field(..., description="Person count per zone")
    processing_time_ms: float = Field(..., description="Processing time in milliseconds")
    metadata: Dict[str, Any] = Field(default_factory=dict, description="Additional metadata")


class HistoricalDataRequest(BaseModel):
    """Request model for historical pose data."""
    
    start_time: datetime = Field(..., description="Start time for data query")
    end_time: datetime = Field(..., description="End time for data query")
    zone_ids: Optional[List[str]] = Field(
        default=None,
        description="Filter by specific zones"
    )
    aggregation_interval: Optional[int] = Field(
        default=300,
        ge=60,
        le=3600,
        description="Aggregation interval in seconds"
    )
    include_raw_data: bool = Field(
        default=False,
        description="Include raw detection data"
    )


# Endpoints
@router.get("/current", response_model=PoseEstimationResponse)
async def get_current_pose_estimation(
    request: PoseEstimationRequest = Depends(),
    pose_service: PoseService = Depends(get_pose_service),
    current_user: Optional[Dict] = Depends(get_current_user)
):
    """Get current pose estimation from WiFi signals."""
    try:
        logger.info(f"Processing pose estimation request from user: {current_user.get('id') if current_user else 'anonymous'}")
        
        # Get current pose estimation
        result = await pose_service.estimate_poses(
            zone_ids=request.zone_ids,
            confidence_threshold=request.confidence_threshold,
            max_persons=request.max_persons,
            include_keypoints=request.include_keypoints,
            include_segmentation=request.include_segmentation
        )
        
        return PoseEstimationResponse(**result)
        
    except Exception as e:
        logger.error(f"Error in pose estimation: {e}")
        raise HTTPException(
            status_code=500,
            detail=f"Pose estimation failed: {str(e)}"
        )


@router.post("/analyze", response_model=PoseEstimationResponse)
async def analyze_pose_data(
    request: PoseEstimationRequest,
    background_tasks: BackgroundTasks,
    pose_service: PoseService = Depends(get_pose_service),
    current_user: Dict = Depends(require_auth)
):
    """Trigger pose analysis with custom parameters."""
    try:
        logger.info(f"Custom pose analysis requested by user: {current_user['id']}")
        
        # Trigger analysis
        result = await pose_service.analyze_with_params(
            zone_ids=request.zone_ids,
            confidence_threshold=request.confidence_threshold,
            max_persons=request.max_persons,
            include_keypoints=request.include_keypoints,
            include_segmentation=request.include_segmentation
        )
        
        # Schedule background processing if needed
        if request.include_segmentation:
            background_tasks.add_task(
                pose_service.process_segmentation_data,
                result["frame_id"]
            )
        
        return PoseEstimationResponse(**result)
        
    except Exception as e:
        logger.error(f"Error in pose analysis: {e}")
        raise HTTPException(
            status_code=500,
            detail=f"Pose analysis failed: {str(e)}"
        )


@router.get("/zones/{zone_id}/occupancy")
async def get_zone_occupancy(
    zone_id: str,
    pose_service: PoseService = Depends(get_pose_service),
    current_user: Optional[Dict] = Depends(get_current_user)
):
    """Get current occupancy for a specific zone."""
    try:
        occupancy = await pose_service.get_zone_occupancy(zone_id)
        
        if occupancy is None:
            raise HTTPException(
                status_code=404,
                detail=f"Zone '{zone_id}' not found"
            )
        
        return {
            "zone_id": zone_id,
            "current_occupancy": occupancy["count"],
            "max_occupancy": occupancy.get("max_occupancy"),
            "persons": occupancy["persons"],
            "timestamp": occupancy["timestamp"]
        }
        
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error getting zone occupancy: {e}")
        raise HTTPException(
            status_code=500,
            detail=f"Failed to get zone occupancy: {str(e)}"
        )


@router.get("/zones/summary")
async def get_zones_summary(
    pose_service: PoseService = Depends(get_pose_service),
    current_user: Optional[Dict] = Depends(get_current_user)
):
    """Get occupancy summary for all zones."""
    try:
        summary = await pose_service.get_zones_summary()
        
        return {
            "timestamp": datetime.utcnow(),
            "total_persons": summary["total_persons"],
            "zones": summary["zones"],
            "active_zones": summary["active_zones"]
        }
        
    except Exception as e:
        logger.error(f"Error getting zones summary: {e}")
        raise HTTPException(
            status_code=500,
            detail=f"Failed to get zones summary: {str(e)}"
        )


@router.post("/historical")
async def get_historical_data(
    request: HistoricalDataRequest,
    pose_service: PoseService = Depends(get_pose_service),
    current_user: Dict = Depends(require_auth)
):
    """Get historical pose estimation data."""
    try:
        # Validate time range
        if request.end_time <= request.start_time:
            raise HTTPException(
                status_code=400,
                detail="End time must be after start time"
            )
        
        # Limit query range to prevent excessive data
        max_range = timedelta(days=7)
        if request.end_time - request.start_time > max_range:
            raise HTTPException(
                status_code=400,
                detail="Query range cannot exceed 7 days"
            )
        
        data = await pose_service.get_historical_data(
            start_time=request.start_time,
            end_time=request.end_time,
            zone_ids=request.zone_ids,
            aggregation_interval=request.aggregation_interval,
            include_raw_data=request.include_raw_data
        )
        
        return {
            "query": {
                "start_time": request.start_time,
                "end_time": request.end_time,
                "zone_ids": request.zone_ids,
                "aggregation_interval": request.aggregation_interval
            },
            "data": data["aggregated_data"],
            "raw_data": data.get("raw_data") if request.include_raw_data else None,
            "total_records": data["total_records"]
        }
        
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error getting historical data: {e}")
        raise HTTPException(
            status_code=500,
            detail=f"Failed to get historical data: {str(e)}"
        )


@router.get("/activities")
async def get_detected_activities(
    zone_id: Optional[str] = Query(None, description="Filter by zone ID"),
    limit: int = Query(10, ge=1, le=100, description="Maximum number of activities"),
    pose_service: PoseService = Depends(get_pose_service),
    current_user: Optional[Dict] = Depends(get_current_user)
):
    """Get recently detected activities."""
    try:
        activities = await pose_service.get_recent_activities(
            zone_id=zone_id,
            limit=limit
        )
        
        return {
            "activities": activities,
            "total_count": len(activities),
            "zone_id": zone_id
        }
        
    except Exception as e:
        logger.error(f"Error getting activities: {e}")
        raise HTTPException(
            status_code=500,
            detail=f"Failed to get activities: {str(e)}"
        )


@router.post("/calibrate")
async def calibrate_pose_system(
    background_tasks: BackgroundTasks,
    pose_service: PoseService = Depends(get_pose_service),
    hardware_service: HardwareService = Depends(get_hardware_service),
    current_user: Dict = Depends(require_auth)
):
    """Calibrate the pose estimation system."""
    try:
        logger.info(f"Pose system calibration initiated by user: {current_user['id']}")
        
        # Check if calibration is already in progress
        if await pose_service.is_calibrating():
            raise HTTPException(
                status_code=409,
                detail="Calibration already in progress"
            )
        
        # Start calibration process
        calibration_id = await pose_service.start_calibration()
        
        # Schedule background calibration task
        background_tasks.add_task(
            pose_service.run_calibration,
            calibration_id
        )
        
        return {
            "calibration_id": calibration_id,
            "status": "started",
            "estimated_duration_minutes": 5,
            "message": "Calibration process started"
        }
        
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error starting calibration: {e}")
        raise HTTPException(
            status_code=500,
            detail=f"Failed to start calibration: {str(e)}"
        )


@router.get("/calibration/status")
async def get_calibration_status(
    pose_service: PoseService = Depends(get_pose_service),
    current_user: Dict = Depends(require_auth)
):
    """Get current calibration status."""
    try:
        status = await pose_service.get_calibration_status()
        
        return {
            "is_calibrating": status["is_calibrating"],
            "calibration_id": status.get("calibration_id"),
            "progress_percent": status.get("progress_percent", 0),
            "current_step": status.get("current_step"),
            "estimated_remaining_minutes": status.get("estimated_remaining_minutes"),
            "last_calibration": status.get("last_calibration")
        }
        
    except Exception as e:
        logger.error(f"Error getting calibration status: {e}")
        raise HTTPException(
            status_code=500,
            detail=f"Failed to get calibration status: {str(e)}"
        )


@router.get("/stats")
async def get_pose_statistics(
    hours: int = Query(24, ge=1, le=168, description="Hours of data to analyze"),
    pose_service: PoseService = Depends(get_pose_service),
    current_user: Optional[Dict] = Depends(get_current_user)
):
    """Get pose estimation statistics."""
    try:
        end_time = datetime.utcnow()
        start_time = end_time - timedelta(hours=hours)
        
        stats = await pose_service.get_statistics(
            start_time=start_time,
            end_time=end_time
        )
        
        return {
            "period": {
                "start_time": start_time,
                "end_time": end_time,
                "hours": hours
            },
            "statistics": stats
        }
        
    except Exception as e:
        logger.error(f"Error getting statistics: {e}")
        raise HTTPException(
            status_code=500,
            detail=f"Failed to get statistics: {str(e)}"
        )