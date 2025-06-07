// Live Demo Tab Component

import { poseService } from '../services/pose.service.js';
import { streamService } from '../services/stream.service.js';

export class LiveDemoTab {
  constructor(containerElement) {
    this.container = containerElement;
    this.isRunning = false;
    this.streamConnection = null;
    this.poseSubscription = null;
    this.signalCanvas = null;
    this.poseCanvas = null;
    this.signalCtx = null;
    this.poseCtx = null;
    this.animationFrame = null;
    this.signalTime = 0;
    this.poseData = null;
  }

  // Initialize component
  init() {
    this.setupCanvases();
    this.setupControls();
    this.initializeDisplays();
  }

  // Set up canvases
  setupCanvases() {
    this.signalCanvas = this.container.querySelector('#signalCanvas');
    this.poseCanvas = this.container.querySelector('#poseCanvas');
    
    if (this.signalCanvas) {
      this.signalCtx = this.signalCanvas.getContext('2d');
    }
    
    if (this.poseCanvas) {
      this.poseCtx = this.poseCanvas.getContext('2d');
    }
  }

  // Set up control buttons
  setupControls() {
    const startButton = this.container.querySelector('#startDemo');
    const stopButton = this.container.querySelector('#stopDemo');
    
    if (startButton) {
      startButton.addEventListener('click', () => this.startDemo());
    }
    
    if (stopButton) {
      stopButton.addEventListener('click', () => this.stopDemo());
    }
  }

  // Initialize displays
  initializeDisplays() {
    // Initialize signal canvas
    if (this.signalCtx) {
      this.signalCtx.fillStyle = 'rgba(0, 0, 0, 0.2)';
      this.signalCtx.fillRect(0, 0, this.signalCanvas.width, this.signalCanvas.height);
    }
    
    // Initialize pose canvas
    if (this.poseCtx) {
      this.poseCtx.fillStyle = 'rgba(0, 0, 0, 0.2)';
      this.poseCtx.fillRect(0, 0, this.poseCanvas.width, this.poseCanvas.height);
    }
  }

  // Start demo
  async startDemo() {
    if (this.isRunning) return;
    
    try {
      // Update UI
      this.isRunning = true;
      this.updateControls();
      this.updateStatus('Starting...', 'info');
      
      // Check stream status
      const streamStatus = await streamService.getStatus();
      
      if (!streamStatus.is_active) {
        // Try to start streaming
        await streamService.start();
      }
      
      // Start pose stream
      this.streamConnection = poseService.startPoseStream({
        minConfidence: 0.5,
        maxFps: 30
      });
      
      // Subscribe to pose updates
      this.poseSubscription = poseService.subscribeToPoseUpdates(update => {
        this.handlePoseUpdate(update);
      });
      
      // Start animations
      this.startAnimations();
      
      // Update status
      this.updateStatus('Running', 'success');
      
    } catch (error) {
      console.error('Failed to start demo:', error);
      this.updateStatus('Failed to start', 'error');
      this.stopDemo();
    }
  }

  // Stop demo
  stopDemo() {
    if (!this.isRunning) return;
    
    // Update UI
    this.isRunning = false;
    this.updateControls();
    this.updateStatus('Stopped', 'info');
    
    // Stop pose stream
    if (this.poseSubscription) {
      this.poseSubscription();
      this.poseSubscription = null;
    }
    
    poseService.stopPoseStream();
    this.streamConnection = null;
    
    // Stop animations
    if (this.animationFrame) {
      cancelAnimationFrame(this.animationFrame);
      this.animationFrame = null;
    }
  }

  // Update controls
  updateControls() {
    const startButton = this.container.querySelector('#startDemo');
    const stopButton = this.container.querySelector('#stopDemo');
    
    if (startButton) {
      startButton.disabled = this.isRunning;
    }
    
    if (stopButton) {
      stopButton.disabled = !this.isRunning;
    }
  }

  // Update status display
  updateStatus(text, type) {
    const statusElement = this.container.querySelector('#demoStatus');
    if (statusElement) {
      statusElement.textContent = text;
      statusElement.className = `status status--${type}`;
    }
  }

  // Handle pose updates
  handlePoseUpdate(update) {
    switch (update.type) {
      case 'connected':
        console.log('Pose stream connected');
        break;
        
      case 'pose_update':
        this.poseData = update.data;
        this.updateMetrics(update.data);
        break;
        
      case 'error':
        console.error('Pose stream error:', update.error);
        this.updateStatus('Stream error', 'error');
        break;
        
      case 'disconnected':
        console.log('Pose stream disconnected');
        if (this.isRunning) {
          this.updateStatus('Disconnected', 'warning');
        }
        break;
    }
  }

  // Update metrics display
  updateMetrics(poseData) {
    if (!poseData) return;
    
    // Update signal strength (simulated based on detection confidence)
    const signalStrength = this.container.querySelector('#signalStrength');
    if (signalStrength) {
      const strength = poseData.persons?.length > 0 
        ? -45 - Math.random() * 10 
        : -55 - Math.random() * 10;
      signalStrength.textContent = `${strength.toFixed(0)} dBm`;
    }
    
    // Update latency
    const latency = this.container.querySelector('#latency');
    if (latency && poseData.processing_time) {
      latency.textContent = `${poseData.processing_time.toFixed(0)} ms`;
    }
    
    // Update person count
    const personCount = this.container.querySelector('#personCount');
    if (personCount) {
      personCount.textContent = poseData.persons?.length || 0;
    }
    
    // Update confidence
    const confidence = this.container.querySelector('#confidence');
    if (confidence && poseData.persons?.length > 0) {
      const avgConfidence = poseData.persons.reduce((sum, p) => sum + p.confidence, 0) 
        / poseData.persons.length * 100;
      confidence.textContent = `${avgConfidence.toFixed(1)}%`;
    }
    
    // Update keypoints
    const keypoints = this.container.querySelector('#keypoints');
    if (keypoints && poseData.persons?.length > 0) {
      const totalKeypoints = poseData.persons[0].keypoints?.length || 0;
      const detectedKeypoints = poseData.persons[0].keypoints?.filter(kp => kp.confidence > 0.5).length || 0;
      keypoints.textContent = `${detectedKeypoints}/${totalKeypoints}`;
    }
  }

  // Start animations
  startAnimations() {
    const animate = () => {
      if (!this.isRunning) return;
      
      // Update signal visualization
      this.updateSignalVisualization();
      
      // Update pose visualization
      this.updatePoseVisualization();
      
      this.animationFrame = requestAnimationFrame(animate);
    };
    
    animate();
  }

  // Update signal visualization
  updateSignalVisualization() {
    if (!this.signalCtx) return;
    
    const ctx = this.signalCtx;
    const width = this.signalCanvas.width;
    const height = this.signalCanvas.height;
    
    // Clear canvas with fade effect
    ctx.fillStyle = 'rgba(0, 0, 0, 0.1)';
    ctx.fillRect(0, 0, width, height);
    
    // Draw amplitude signal
    ctx.beginPath();
    ctx.strokeStyle = '#1FB8CD';
    ctx.lineWidth = 2;
    
    for (let x = 0; x < width; x++) {
      const hasData = this.poseData?.persons?.length > 0;
      const amplitude = hasData ? 30 : 10;
      const frequency = hasData ? 0.05 : 0.02;
      
      const y = height / 2 + 
        Math.sin(x * frequency + this.signalTime) * amplitude +
        Math.sin(x * 0.02 + this.signalTime * 1.5) * 15;
      
      if (x === 0) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    }
    
    ctx.stroke();
    
    // Draw phase signal
    ctx.beginPath();
    ctx.strokeStyle = '#FFC185';
    ctx.lineWidth = 2;
    
    for (let x = 0; x < width; x++) {
      const hasData = this.poseData?.persons?.length > 0;
      const amplitude = hasData ? 25 : 15;
      
      const y = height / 2 + 
        Math.cos(x * 0.03 + this.signalTime * 0.8) * amplitude +
        Math.cos(x * 0.01 + this.signalTime * 0.5) * 20;
      
      if (x === 0) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    }
    
    ctx.stroke();
    
    this.signalTime += 0.05;
  }

  // Update pose visualization
  updatePoseVisualization() {
    if (!this.poseCtx || !this.poseData) return;
    
    const ctx = this.poseCtx;
    const width = this.poseCanvas.width;
    const height = this.poseCanvas.height;
    
    // Clear canvas
    ctx.clearRect(0, 0, width, height);
    ctx.fillStyle = 'rgba(0, 0, 0, 0.2)';
    ctx.fillRect(0, 0, width, height);
    
    // Draw each detected person
    if (this.poseData.persons) {
      this.poseData.persons.forEach((person, index) => {
        this.drawPerson(ctx, person, index);
      });
    }
  }

  // Draw a person's pose
  drawPerson(ctx, person, index) {
    if (!person.keypoints) return;
    
    // Define COCO keypoint connections
    const connections = [
      [0, 1], [0, 2], [1, 3], [2, 4],  // Head
      [5, 6], [5, 7], [7, 9], [6, 8], [8, 10],  // Arms
      [5, 11], [6, 12], [11, 12],  // Body
      [11, 13], [13, 15], [12, 14], [14, 16]  // Legs
    ];
    
    // Scale keypoints to canvas
    const scale = Math.min(this.poseCanvas.width, this.poseCanvas.height) / 2;
    const offsetX = this.poseCanvas.width / 2;
    const offsetY = this.poseCanvas.height / 2;
    
    // Draw skeleton connections
    ctx.strokeStyle = `hsl(${index * 60}, 70%, 50%)`;
    ctx.lineWidth = 3;
    
    connections.forEach(([i, j]) => {
      const kp1 = person.keypoints[i];
      const kp2 = person.keypoints[j];
      
      if (kp1 && kp2 && kp1.confidence > 0.3 && kp2.confidence > 0.3) {
        ctx.beginPath();
        ctx.moveTo(kp1.x * scale + offsetX, kp1.y * scale + offsetY);
        ctx.lineTo(kp2.x * scale + offsetX, kp2.y * scale + offsetY);
        ctx.stroke();
      }
    });
    
    // Draw keypoints
    ctx.fillStyle = `hsl(${index * 60}, 70%, 60%)`;
    
    person.keypoints.forEach(kp => {
      if (kp.confidence > 0.3) {
        ctx.beginPath();
        ctx.arc(
          kp.x * scale + offsetX, 
          kp.y * scale + offsetY, 
          5, 
          0, 
          Math.PI * 2
        );
        ctx.fill();
      }
    });
    
    // Draw confidence label
    ctx.fillStyle = 'white';
    ctx.font = '12px monospace';
    ctx.fillText(
      `Person ${index + 1}: ${(person.confidence * 100).toFixed(1)}%`,
      10,
      20 + index * 20
    );
  }

  // Clean up
  dispose() {
    this.stopDemo();
  }
}