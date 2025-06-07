// WiFi DensePose Application JavaScript

document.addEventListener('DOMContentLoaded', function() {
    // Initialize tabs
    initTabs();
    
    // Initialize hardware visualization
    initHardware();
    
    // Initialize demo simulation
    initDemo();
    
    // Initialize architecture interaction
    initArchitecture();
});

// Tab switching functionality
function initTabs() {
    const tabs = document.querySelectorAll('.nav-tab');
    const tabContents = document.querySelectorAll('.tab-content');
    
    tabs.forEach(tab => {
        tab.addEventListener('click', () => {
            // Get the tab id
            const tabId = tab.getAttribute('data-tab');
            
            // Remove active class from all tabs and contents
            tabs.forEach(t => t.classList.remove('active'));
            tabContents.forEach(c => c.classList.remove('active'));
            
            // Add active class to current tab and content
            tab.classList.add('active');
            document.getElementById(tabId).classList.add('active');
        });
    });
}

// Hardware panel functionality
function initHardware() {
    // Antenna interaction
    const antennas = document.querySelectorAll('.antenna');
    
    antennas.forEach(antenna => {
        antenna.addEventListener('click', () => {
            antenna.classList.toggle('active');
            updateCSIDisplay();
        });
    });
    
    // Start CSI simulation
    updateCSIDisplay();
    setInterval(updateCSIDisplay, 1000);
}

// Update CSI display with random values
function updateCSIDisplay() {
    const activeAntennas = document.querySelectorAll('.antenna.active');
    const isActive = activeAntennas.length > 0;
    
    // Only update if at least one antenna is active
    if (isActive) {
        const amplitudeFill = document.querySelector('.csi-fill.amplitude');
        const phaseFill = document.querySelector('.csi-fill.phase');
        const amplitudeValue = document.querySelector('.csi-row:first-child .csi-value');
        const phaseValue = document.querySelector('.csi-row:last-child .csi-value');
        
        // Generate random values
        const amplitude = (Math.random() * 0.4 + 0.5).toFixed(2); // Between 0.5 and 0.9
        const phase = (Math.random() * 1.5 + 0.5).toFixed(1); // Between 0.5 and 2.0
        
        // Update the display
        amplitudeFill.style.width = `${amplitude * 100}%`;
        phaseFill.style.width = `${phase * 50}%`;
        amplitudeValue.textContent = amplitude;
        phaseValue.textContent = `${phase}π`;
    }
}

// Demo functionality
function initDemo() {
    const startButton = document.getElementById('startDemo');
    const stopButton = document.getElementById('stopDemo');
    const demoStatus = document.getElementById('demoStatus');
    const signalCanvas = document.getElementById('signalCanvas');
    const poseCanvas = document.getElementById('poseCanvas');
    const signalStrength = document.getElementById('signalStrength');
    const latency = document.getElementById('latency');
    const personCount = document.getElementById('personCount');
    const confidence = document.getElementById('confidence');
    const keypoints = document.getElementById('keypoints');
    
    let demoRunning = false;
    let animationFrameId = null;
    let signalCtx = signalCanvas.getContext('2d');
    let poseCtx = poseCanvas.getContext('2d');
    
    // Initialize canvas contexts
    signalCtx.fillStyle = 'rgba(0, 0, 0, 0.2)';
    signalCtx.fillRect(0, 0, signalCanvas.width, signalCanvas.height);
    
    poseCtx.fillStyle = 'rgba(0, 0, 0, 0.2)';
    poseCtx.fillRect(0, 0, poseCanvas.width, poseCanvas.height);
    
    // Start demo button
    startButton.addEventListener('click', () => {
        if (!demoRunning) {
            demoRunning = true;
            startButton.disabled = true;
            stopButton.disabled = false;
            demoStatus.textContent = 'Running';
            demoStatus.className = 'status status--success';
            
            // Start the animations
            startSignalAnimation();
            startPoseAnimation();
            
            // Update metrics with random values
            updateDemoMetrics();
        }
    });
    
    // Stop demo button
    stopButton.addEventListener('click', () => {
        if (demoRunning) {
            demoRunning = false;
            startButton.disabled = false;
            stopButton.disabled = true;
            demoStatus.textContent = 'Stopped';
            demoStatus.className = 'status status--info';
            
            // Stop the animations
            if (animationFrameId) {
                cancelAnimationFrame(animationFrameId);
            }
        }
    });
    
    // Signal animation
    function startSignalAnimation() {
        let time = 0;
        const fps = 30;
        const interval = 1000 / fps;
        let then = Date.now();
        
        function animate() {
            if (!demoRunning) return;
            
            const now = Date.now();
            const elapsed = now - then;
            
            if (elapsed > interval) {
                then = now - (elapsed % interval);
                
                // Clear canvas
                signalCtx.clearRect(0, 0, signalCanvas.width, signalCanvas.height);
                signalCtx.fillStyle = 'rgba(0, 0, 0, 0.2)';
                signalCtx.fillRect(0, 0, signalCanvas.width, signalCanvas.height);
                
                // Draw amplitude signal
                signalCtx.beginPath();
                signalCtx.strokeStyle = '#1FB8CD';
                signalCtx.lineWidth = 2;
                
                for (let x = 0; x < signalCanvas.width; x++) {
                    const y = signalCanvas.height / 2 + 
                        Math.sin(x * 0.05 + time) * 30 +
                        Math.sin(x * 0.02 + time * 1.5) * 15;
                    
                    if (x === 0) {
                        signalCtx.moveTo(x, y);
                    } else {
                        signalCtx.lineTo(x, y);
                    }
                }
                
                signalCtx.stroke();
                
                // Draw phase signal
                signalCtx.beginPath();
                signalCtx.strokeStyle = '#FFC185';
                signalCtx.lineWidth = 2;
                
                for (let x = 0; x < signalCanvas.width; x++) {
                    const y = signalCanvas.height / 2 + 
                        Math.cos(x * 0.03 + time * 0.8) * 20 +
                        Math.cos(x * 0.01 + time * 0.5) * 25;
                    
                    if (x === 0) {
                        signalCtx.moveTo(x, y);
                    } else {
                        signalCtx.lineTo(x, y);
                    }
                }
                
                signalCtx.stroke();
                
                time += 0.05;
            }
            
            animationFrameId = requestAnimationFrame(animate);
        }
        
        animate();
    }
    
    // Human pose animation
    function startPoseAnimation() {
        // Create a human wireframe model with keypoints
        const keyPoints = [
            { x: 200, y: 70 },  // Head
            { x: 200, y: 100 }, // Neck
            { x: 200, y: 150 }, // Torso
            { x: 160, y: 100 }, // Left shoulder
            { x: 120, y: 130 }, // Left elbow
            { x: 100, y: 160 }, // Left hand
            { x: 240, y: 100 }, // Right shoulder
            { x: 280, y: 130 }, // Right elbow
            { x: 300, y: 160 }, // Right hand
            { x: 180, y: 200 }, // Left hip
            { x: 170, y: 250 }, // Left knee
            { x: 160, y: 290 }, // Left foot
            { x: 220, y: 200 }, // Right hip
            { x: 230, y: 250 }, // Right knee
            { x: 240, y: 290 }, // Right foot
        ];
        
        // Connections between points
        const connections = [
            [0, 1],  // Head to neck
            [1, 2],  // Neck to torso
            [1, 3],  // Neck to left shoulder
            [3, 4],  // Left shoulder to left elbow
            [4, 5],  // Left elbow to left hand
            [1, 6],  // Neck to right shoulder
            [6, 7],  // Right shoulder to right elbow
            [7, 8],  // Right elbow to right hand
            [2, 9],  // Torso to left hip
            [9, 10], // Left hip to left knee
            [10, 11], // Left knee to left foot
            [2, 12], // Torso to right hip
            [12, 13], // Right hip to right knee
            [13, 14], // Right knee to right foot
            [9, 12]  // Left hip to right hip
        ];
        
        let time = 0;
        const fps = 30;
        const interval = 1000 / fps;
        let then = Date.now();
        
        function animate() {
            if (!demoRunning) return;
            
            const now = Date.now();
            const elapsed = now - then;
            
            if (elapsed > interval) {
                then = now - (elapsed % interval);
                
                // Clear canvas
                poseCtx.clearRect(0, 0, poseCanvas.width, poseCanvas.height);
                poseCtx.fillStyle = 'rgba(0, 0, 0, 0.2)';
                poseCtx.fillRect(0, 0, poseCanvas.width, poseCanvas.height);
                
                // Animate keypoints with subtle movement
                const animatedPoints = keyPoints.map((point, index) => {
                    // Add subtle movement based on position
                    const xOffset = Math.sin(time + index * 0.2) * 2;
                    const yOffset = Math.cos(time + index * 0.2) * 2;
                    
                    return {
                        x: point.x + xOffset,
                        y: point.y + yOffset
                    };
                });
                
                // Draw connections (skeleton)
                poseCtx.strokeStyle = '#1FB8CD';
                poseCtx.lineWidth = 3;
                
                connections.forEach(([i, j]) => {
                    poseCtx.beginPath();
                    poseCtx.moveTo(animatedPoints[i].x, animatedPoints[i].y);
                    poseCtx.lineTo(animatedPoints[j].x, animatedPoints[j].y);
                    poseCtx.stroke();
                });
                
                // Draw keypoints
                poseCtx.fillStyle = '#FFC185';
                
                animatedPoints.forEach(point => {
                    poseCtx.beginPath();
                    poseCtx.arc(point.x, point.y, 5, 0, Math.PI * 2);
                    poseCtx.fill();
                });
                
                // Draw body segments (simplified DensePose representation)
                drawBodySegments(poseCtx, animatedPoints);
                
                time += 0.05;
            }
            
            animationFrameId = requestAnimationFrame(animate);
        }
        
        animate();
    }
    
    // Draw body segments for DensePose visualization
    function drawBodySegments(ctx, points) {
        // Define simplified body segments
        const segments = [
            [0, 1, 6, 3], // Head and shoulders
            [1, 2, 12, 9], // Torso
            [3, 4, 5, 3], // Left arm
            [6, 7, 8, 6], // Right arm
            [9, 10, 11, 9], // Left leg
            [12, 13, 14, 12] // Right leg
        ];
        
        ctx.globalAlpha = 0.2;
        
        segments.forEach((segment, index) => {
            const gradient = ctx.createLinearGradient(
                points[segment[0]].x, points[segment[0]].y,
                points[segment[2]].x, points[segment[2]].y
            );
            
            gradient.addColorStop(0, '#1FB8CD');
            gradient.addColorStop(1, '#FFC185');
            
            ctx.fillStyle = gradient;
            ctx.beginPath();
            ctx.moveTo(points[segment[0]].x, points[segment[0]].y);
            
            // Connect the points in the segment
            for (let i = 1; i < segment.length; i++) {
                ctx.lineTo(points[segment[i]].x, points[segment[i]].y);
            }
            
            ctx.closePath();
            ctx.fill();
        });
        
        ctx.globalAlpha = 1.0;
    }
    
    // Update demo metrics
    function updateDemoMetrics() {
        if (!demoRunning) return;
        
        // Update with random values
        const strength = Math.floor(Math.random() * 10) - 50;
        const lat = Math.floor(Math.random() * 8) + 8;
        const persons = Math.floor(Math.random() * 2) + 1;
        const conf = (Math.random() * 10 + 80).toFixed(1);
        
        signalStrength.textContent = `${strength} dBm`;
        latency.textContent = `${lat} ms`;
        personCount.textContent = persons;
        confidence.textContent = `${conf}%`;
        
        // Schedule next update
        setTimeout(updateDemoMetrics, 2000);
    }
}

// Architecture interaction
function initArchitecture() {
    const stepCards = document.querySelectorAll('.step-card');
    
    stepCards.forEach(card => {
        card.addEventListener('click', () => {
            // Get step number
            const step = card.getAttribute('data-step');
            
            // Remove active class from all steps
            stepCards.forEach(s => s.classList.remove('highlight'));
            
            // Add active class to current step
            card.classList.add('highlight');
        });
    });
}