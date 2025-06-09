// Pose Renderer Utility for WiFi-DensePose UI

export class PoseRenderer {
  constructor(canvas, options = {}) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d');
    this.config = {
      // Rendering modes
      mode: 'skeleton', // 'skeleton', 'keypoints', 'heatmap', 'dense'
      
      // Visual settings
      showKeypoints: true,
      showSkeleton: true,
      showBoundingBox: false,
      showConfidence: true,
      showZones: true,
      showDebugInfo: false,
      
      // Colors
      skeletonColor: '#00ff00',
      keypointColor: '#ff0000',
      boundingBoxColor: '#0000ff',
      confidenceColor: '#ffffff',
      zoneColor: '#ffff00',
      
      // Sizes
      keypointRadius: 4,
      skeletonWidth: 2,
      boundingBoxWidth: 2,
      fontSize: 12,
      
      // Thresholds
      confidenceThreshold: 0.3,
      keypointConfidenceThreshold: 0.1,
      
      // Performance
      enableSmoothing: true,
      maxFps: 30,
      
      ...options
    };
    
    this.logger = this.createLogger();
    this.performanceMetrics = {
      frameCount: 0,
      lastFrameTime: 0,
      averageFps: 0,
      renderTime: 0
    };
    
    // Pose skeleton connections (COCO format, 0-indexed)
    this.skeletonConnections = [
      [15, 13], [13, 11], [16, 14], [14, 12], [11, 12], // Head
      [5, 11], [6, 12], [5, 6], // Torso
      [5, 7], [6, 8], [7, 9], [8, 10], // Arms
      [11, 13], [12, 14], [13, 15], [14, 16] // Legs
    ];
    
    // Initialize rendering context
    this.initializeContext();
  }

  createLogger() {
    return {
      debug: (...args) => console.debug('[RENDERER-DEBUG]', new Date().toISOString(), ...args),
      info: (...args) => console.info('[RENDERER-INFO]', new Date().toISOString(), ...args),
      warn: (...args) => console.warn('[RENDERER-WARN]', new Date().toISOString(), ...args),
      error: (...args) => console.error('[RENDERER-ERROR]', new Date().toISOString(), ...args)
    };
  }

  initializeContext() {
    this.ctx.imageSmoothingEnabled = this.config.enableSmoothing;
    this.ctx.font = `${this.config.fontSize}px Arial`;
    this.ctx.textAlign = 'left';
    this.ctx.textBaseline = 'top';
  }

  // Main render method
  render(poseData, metadata = {}) {
    const startTime = performance.now();
    
    try {
      // Clear canvas
      this.clearCanvas();
      
      console.log('🎨 [RENDERER] Rendering pose data:', poseData);
      
      if (!poseData || !poseData.persons) {
        console.log('⚠️ [RENDERER] No pose data or persons array');
        this.renderNoDataMessage();
        return;
      }
      
      console.log(`👥 [RENDERER] Found ${poseData.persons.length} persons to render`);

      // Render based on mode
      switch (this.config.mode) {
        case 'skeleton':
          this.renderSkeletonMode(poseData, metadata);
          break;
        case 'keypoints':
          this.renderKeypointsMode(poseData, metadata);
          break;
        case 'heatmap':
          this.renderHeatmapMode(poseData, metadata);
          break;
        case 'dense':
          this.renderDenseMode(poseData, metadata);
          break;
        default:
          this.renderSkeletonMode(poseData, metadata);
      }

      // Render debug information if enabled
      if (this.config.showDebugInfo) {
        this.renderDebugInfo(poseData, metadata);
      }

      // Update performance metrics
      this.updatePerformanceMetrics(startTime);
      
    } catch (error) {
      this.logger.error('Render error', { error: error.message });
      this.renderErrorMessage(error.message);
    }
  }

  clearCanvas() {
    this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
    
    // Optional: Add background
    if (this.config.backgroundColor) {
      this.ctx.fillStyle = this.config.backgroundColor;
      this.ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);
    }
  }

  // Skeleton rendering mode
  renderSkeletonMode(poseData, metadata) {
    const persons = poseData.persons || [];
    
    console.log(`🦴 [RENDERER] Skeleton mode: processing ${persons.length} persons`);
    
    persons.forEach((person, index) => {
      console.log(`👤 [RENDERER] Person ${index}:`, person);
      
      if (person.confidence < this.config.confidenceThreshold) {
        console.log(`❌ [RENDERER] Skipping person ${index} - low confidence: ${person.confidence} < ${this.config.confidenceThreshold}`);
        return; // Skip low confidence detections
      }

      console.log(`✅ [RENDERER] Rendering person ${index} with confidence: ${person.confidence}`);

      // Render skeleton connections
      if (this.config.showSkeleton && person.keypoints) {
        console.log(`🦴 [RENDERER] Rendering skeleton for person ${index}`);
        this.renderSkeleton(person.keypoints, person.confidence);
      }

      // Render keypoints
      if (this.config.showKeypoints && person.keypoints) {
        console.log(`🔴 [RENDERER] Rendering keypoints for person ${index}`);
        this.renderKeypoints(person.keypoints, person.confidence);
      }

      // Render bounding box
      if (this.config.showBoundingBox && person.bbox) {
        console.log(`📦 [RENDERER] Rendering bounding box for person ${index}`);
        this.renderBoundingBox(person.bbox, person.confidence, index);
      }

      // Render confidence score
      if (this.config.showConfidence) {
        console.log(`📊 [RENDERER] Rendering confidence score for person ${index}`);
        this.renderConfidenceScore(person, index);
      }
    });

    // Render zones if available
    if (this.config.showZones && poseData.zone_summary) {
      this.renderZones(poseData.zone_summary);
    }
  }

  // Keypoints only mode
  renderKeypointsMode(poseData, metadata) {
    const persons = poseData.persons || [];
    
    persons.forEach((person, index) => {
      if (person.confidence >= this.config.confidenceThreshold && person.keypoints) {
        this.renderKeypoints(person.keypoints, person.confidence, true);
      }
    });
  }

  // Heatmap rendering mode
  renderHeatmapMode(poseData, metadata) {
    // This would render a heatmap visualization
    // For now, fall back to skeleton mode
    this.logger.debug('Heatmap mode not fully implemented, using skeleton mode');
    this.renderSkeletonMode(poseData, metadata);
  }

  // Dense pose rendering mode
  renderDenseMode(poseData, metadata) {
    // This would render dense pose segmentation
    // For now, fall back to skeleton mode
    this.logger.debug('Dense mode not fully implemented, using skeleton mode');
    this.renderSkeletonMode(poseData, metadata);
  }

  // Render skeleton connections
  renderSkeleton(keypoints, confidence) {
    this.skeletonConnections.forEach(([pointA, pointB]) => {
      const keypointA = keypoints[pointA];
      const keypointB = keypoints[pointB];

      if (keypointA && keypointB && 
          keypointA.confidence > this.config.keypointConfidenceThreshold &&
          keypointB.confidence > this.config.keypointConfidenceThreshold) {
        
        const x1 = this.scaleX(keypointA.x);
        const y1 = this.scaleY(keypointA.y);
        const x2 = this.scaleX(keypointB.x);
        const y2 = this.scaleY(keypointB.y);

        // Calculate line confidence based on both keypoints
        const lineConfidence = (keypointA.confidence + keypointB.confidence) / 2;
        
        // Variable line width based on confidence
        const lineWidth = this.config.skeletonWidth + (lineConfidence - 0.5) * 2;
        this.ctx.lineWidth = Math.max(1, Math.min(4, lineWidth));
        
        // Create gradient along the line
        const gradient = this.ctx.createLinearGradient(x1, y1, x2, y2);
        const colorA = this.addAlphaToColor(this.config.skeletonColor, keypointA.confidence);
        const colorB = this.addAlphaToColor(this.config.skeletonColor, keypointB.confidence);
        gradient.addColorStop(0, colorA);
        gradient.addColorStop(1, colorB);
        
        this.ctx.strokeStyle = gradient;
        this.ctx.globalAlpha = Math.min(confidence * 1.2, 1.0);
        
        // Add subtle glow for high confidence connections
        if (lineConfidence > 0.8) {
          this.ctx.shadowColor = this.config.skeletonColor;
          this.ctx.shadowBlur = 3;
        }

        this.ctx.beginPath();
        this.ctx.moveTo(x1, y1);
        this.ctx.lineTo(x2, y2);
        this.ctx.stroke();
        
        // Reset shadow
        this.ctx.shadowBlur = 0;
      }
    });

    this.ctx.globalAlpha = 1.0;
  }

  // Render keypoints
  renderKeypoints(keypoints, confidence, enhancedMode = false) {
    keypoints.forEach((keypoint, index) => {
      if (keypoint.confidence > this.config.keypointConfidenceThreshold) {
        const x = this.scaleX(keypoint.x);
        const y = this.scaleY(keypoint.y);
        
        // Calculate radius based on confidence and keypoint importance
        const baseRadius = this.config.keypointRadius;
        const confidenceRadius = baseRadius + (keypoint.confidence - 0.5) * 2;
        const radius = Math.max(2, Math.min(8, confidenceRadius));
        
        // Set color based on keypoint type or confidence
        if (enhancedMode) {
          this.ctx.fillStyle = this.getKeypointColor(index, keypoint.confidence);
        } else {
          this.ctx.fillStyle = this.config.keypointColor;
        }
        
        // Add glow effect for high confidence keypoints
        if (keypoint.confidence > 0.8) {
          this.ctx.shadowColor = this.ctx.fillStyle;
          this.ctx.shadowBlur = 6;
          this.ctx.shadowOffsetX = 0;
          this.ctx.shadowOffsetY = 0;
        }
        
        this.ctx.globalAlpha = Math.min(1.0, keypoint.confidence + 0.3);
        
        // Draw keypoint with gradient
        const gradient = this.ctx.createRadialGradient(x, y, 0, x, y, radius);
        gradient.addColorStop(0, this.ctx.fillStyle);
        gradient.addColorStop(1, this.addAlphaToColor(this.ctx.fillStyle, 0.3));
        this.ctx.fillStyle = gradient;
        
        this.ctx.beginPath();
        this.ctx.arc(x, y, radius, 0, 2 * Math.PI);
        this.ctx.fill();
        
        // Reset shadow
        this.ctx.shadowBlur = 0;

        // Add keypoint labels in enhanced mode
        if (enhancedMode && this.config.showDebugInfo) {
          this.ctx.fillStyle = this.config.confidenceColor;
          this.ctx.font = '10px Arial';
          this.ctx.fillText(`${index}`, x + radius + 2, y - radius);
        }
      }
    });

    this.ctx.globalAlpha = 1.0;
  }

  // Render bounding box
  renderBoundingBox(bbox, confidence, personIndex) {
    const x = this.scaleX(bbox.x);
    const y = this.scaleY(bbox.y);
    const x2 = this.scaleX(bbox.x + bbox.width);
    const y2 = this.scaleY(bbox.y + bbox.height);
    const width = x2 - x;
    const height = y2 - y;

    this.ctx.strokeStyle = this.config.boundingBoxColor;
    this.ctx.lineWidth = this.config.boundingBoxWidth;
    this.ctx.globalAlpha = confidence;

    this.ctx.strokeRect(x, y, width, height);

    // Add person label
    this.ctx.fillStyle = this.config.boundingBoxColor;
    this.ctx.fillText(`Person ${personIndex + 1}`, x, y - 15);

    this.ctx.globalAlpha = 1.0;
  }

  // Render confidence score
  renderConfidenceScore(person, index) {
    let x, y;
    
    if (person.bbox) {
      x = this.scaleX(person.bbox.x);
      y = this.scaleY(person.bbox.y + person.bbox.height) + 5;
    } else if (person.keypoints && person.keypoints.length > 0) {
      // Use first available keypoint
      const firstKeypoint = person.keypoints.find(kp => kp.confidence > 0);
      if (firstKeypoint) {
        x = this.scaleX(firstKeypoint.x);
        y = this.scaleY(firstKeypoint.y) + 20;
      } else {
        x = 10;
        y = 30 + (index * 20);
      }
    } else {
      x = 10;
      y = 30 + (index * 20);
    }

    this.ctx.fillStyle = this.config.confidenceColor;
    this.ctx.fillText(`Conf: ${(person.confidence * 100).toFixed(1)}%`, x, y);
  }

  // Render zones
  renderZones(zoneSummary) {
    Object.entries(zoneSummary).forEach(([zoneId, count], index) => {
      const y = 10 + (index * 20);
      
      this.ctx.fillStyle = this.config.zoneColor;
      this.ctx.fillText(`Zone ${zoneId}: ${count} person(s)`, 10, y);
    });
  }

  // Render debug information
  renderDebugInfo(poseData, metadata) {
    const debugInfo = [
      `Frame: ${poseData.frame_id || 'N/A'}`,
      `Timestamp: ${poseData.timestamp || 'N/A'}`,
      `Persons: ${poseData.persons?.length || 0}`,
      `Processing: ${poseData.processing_time_ms || 0}ms`,
      `FPS: ${this.performanceMetrics.averageFps.toFixed(1)}`,
      `Render: ${this.performanceMetrics.renderTime.toFixed(1)}ms`
    ];

    const startY = this.canvas.height - (debugInfo.length * 15) - 10;
    
    this.ctx.fillStyle = 'rgba(0, 0, 0, 0.7)';
    this.ctx.fillRect(5, startY - 5, 200, debugInfo.length * 15 + 10);
    
    this.ctx.fillStyle = '#ffffff';
    debugInfo.forEach((info, index) => {
      this.ctx.fillText(info, 10, startY + (index * 15));
    });
  }

  // Render error message
  renderErrorMessage(message) {
    this.ctx.fillStyle = '#ff0000';
    this.ctx.font = '16px Arial';
    this.ctx.textAlign = 'center';
    this.ctx.fillText(
      `Render Error: ${message}`, 
      this.canvas.width / 2, 
      this.canvas.height / 2
    );
    this.ctx.textAlign = 'left';
    this.ctx.font = `${this.config.fontSize}px Arial`;
  }

  // Render no data message
  renderNoDataMessage() {
    this.ctx.fillStyle = '#888888';
    this.ctx.font = '16px Arial';
    this.ctx.textAlign = 'center';
    this.ctx.fillText(
      'No pose data available', 
      this.canvas.width / 2, 
      this.canvas.height / 2
    );
    this.ctx.fillText(
      'Click "Demo" to see test poses', 
      this.canvas.width / 2, 
      this.canvas.height / 2 + 25
    );
    this.ctx.textAlign = 'left';
    this.ctx.font = `${this.config.fontSize}px Arial`;
  }

  // Test method to verify canvas is working
  renderTestShape() {
    console.log('🔧 [RENDERER] Rendering test shape');
    this.clearCanvas();
    
    // Draw a test rectangle
    this.ctx.fillStyle = '#ff0000';
    this.ctx.fillRect(50, 50, 100, 100);
    
    // Draw a test circle
    this.ctx.fillStyle = '#00ff00';
    this.ctx.beginPath();
    this.ctx.arc(250, 100, 50, 0, 2 * Math.PI);
    this.ctx.fill();
    
    // Draw test text
    this.ctx.fillStyle = '#0000ff';
    this.ctx.font = '16px Arial';
    this.ctx.fillText('Canvas Test', 50, 200);
    
    console.log('✅ [RENDERER] Test shape rendered');
  }

  // Utility methods
  scaleX(x) {
    // If x is already in pixel coordinates (> 1), assume it's in the range 0-800
    // If x is normalized (0-1), scale to canvas width
    if (x > 1) {
      // Assume original image width of 800 pixels
      return (x / 800) * this.canvas.width;
    } else {
      return x * this.canvas.width;
    }
  }

  scaleY(y) {
    // If y is already in pixel coordinates (> 1), assume it's in the range 0-600
    // If y is normalized (0-1), scale to canvas height
    if (y > 1) {
      // Assume original image height of 600 pixels
      return (y / 600) * this.canvas.height;
    } else {
      return y * this.canvas.height;
    }
  }

  getKeypointColor(index, confidence) {
    // Color based on body part
    const colors = [
      '#ff0000', '#ff4500', '#ffa500', '#ffff00', '#adff2f', // Head/neck
      '#00ff00', '#00ff7f', '#00ffff', '#0080ff', '#0000ff', // Torso
      '#4000ff', '#8000ff', '#ff00ff', '#ff0080', '#ff0040', // Arms
      '#ff8080', '#ffb380', '#ffe680'  // Legs
    ];
    
    const color = colors[index % colors.length];
    const alpha = Math.floor(confidence * 255).toString(16).padStart(2, '0');
    return color + alpha;
  }

  addAlphaToColor(color, alpha) {
    // Convert hex color to rgba
    if (color.startsWith('#')) {
      const hex = color.slice(1);
      const r = parseInt(hex.slice(0, 2), 16);
      const g = parseInt(hex.slice(2, 4), 16);
      const b = parseInt(hex.slice(4, 6), 16);
      return `rgba(${r}, ${g}, ${b}, ${alpha})`;
    }
    // If already rgba, modify alpha
    if (color.startsWith('rgba')) {
      return color.replace(/[\d\.]+\)$/g, `${alpha})`);
    }
    // If rgb, convert to rgba
    if (color.startsWith('rgb')) {
      return color.replace('rgb', 'rgba').replace(')', `, ${alpha})`);
    }
    return color;
  }

  updatePerformanceMetrics(startTime) {
    const currentTime = performance.now();
    this.performanceMetrics.renderTime = currentTime - startTime;
    this.performanceMetrics.frameCount++;

    if (this.performanceMetrics.lastFrameTime > 0) {
      const deltaTime = currentTime - this.performanceMetrics.lastFrameTime;
      const fps = 1000 / deltaTime;
      
      // Update average FPS using exponential moving average
      if (this.performanceMetrics.averageFps === 0) {
        this.performanceMetrics.averageFps = fps;
      } else {
        this.performanceMetrics.averageFps = 
          (this.performanceMetrics.averageFps * 0.9) + (fps * 0.1);
      }
    }

    this.performanceMetrics.lastFrameTime = currentTime;
  }

  // Configuration methods
  updateConfig(newConfig) {
    this.config = { ...this.config, ...newConfig };
    this.initializeContext();
    this.logger.debug('Renderer configuration updated', { config: this.config });
  }

  setMode(mode) {
    this.config.mode = mode;
    this.logger.info('Render mode changed', { mode });
  }

  // Utility methods for external access
  getPerformanceMetrics() {
    return { ...this.performanceMetrics };
  }

  getConfig() {
    return { ...this.config };
  }

  // Resize handling
  resize(width, height) {
    this.canvas.width = width;
    this.canvas.height = height;
    this.initializeContext();
    this.logger.debug('Canvas resized', { width, height });
  }

  // Export frame as image
  exportFrame(format = 'png') {
    try {
      return this.canvas.toDataURL(`image/${format}`);
    } catch (error) {
      this.logger.error('Failed to export frame', { error: error.message });
      return null;
    }
  }
}

// Static utility methods
export const PoseRendererUtils = {
  // Create default configuration
  createDefaultConfig: () => ({
    mode: 'skeleton',
    showKeypoints: true,
    showSkeleton: true,
    showBoundingBox: false,
    showConfidence: true,
    showZones: true,
    showDebugInfo: false,
    skeletonColor: '#00ff00',
    keypointColor: '#ff0000',
    boundingBoxColor: '#0000ff',
    confidenceColor: '#ffffff',
    zoneColor: '#ffff00',
    keypointRadius: 4,
    skeletonWidth: 2,
    boundingBoxWidth: 2,
    fontSize: 12,
    confidenceThreshold: 0.3,
    keypointConfidenceThreshold: 0.1,
    enableSmoothing: true,
    maxFps: 30
  }),

  // Validate pose data format
  validatePoseData: (poseData) => {
    const errors = [];

    if (!poseData || typeof poseData !== 'object') {
      errors.push('Pose data must be an object');
      return { valid: false, errors };
    }

    if (!Array.isArray(poseData.persons)) {
      errors.push('Pose data must contain a persons array');
    }

    return {
      valid: errors.length === 0,
      errors
    };
  }
};