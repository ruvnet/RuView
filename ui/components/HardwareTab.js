// Hardware Tab Component

export class HardwareTab {
  constructor(containerElement) {
    this.container = containerElement;
    this.antennas = [];
    this.csiUpdateInterval = null;
    this.isActive = false;
  }

  // Initialize component
  init() {
    this.setupAntennas();
    this.startCSISimulation();
  }

  // Set up antenna interactions
  setupAntennas() {
    this.antennas = Array.from(this.container.querySelectorAll('.antenna'));
    
    this.antennas.forEach(antenna => {
      antenna.addEventListener('click', () => {
        antenna.classList.toggle('active');
        this.updateCSIDisplay();
      });
    });
  }

  // Start CSI simulation
  startCSISimulation() {
    // Initial update
    this.updateCSIDisplay();
    
    // Set up periodic updates
    this.csiUpdateInterval = setInterval(() => {
      if (this.hasActiveAntennas()) {
        this.updateCSIDisplay();
      }
    }, 1000);
  }

  // Check if any antennas are active
  hasActiveAntennas() {
    return this.antennas.some(antenna => antenna.classList.contains('active'));
  }

  // Update CSI display
  updateCSIDisplay() {
    const activeAntennas = this.antennas.filter(a => a.classList.contains('active'));
    const isActive = activeAntennas.length > 0;
    
    // Get display elements
    const amplitudeFill = this.container.querySelector('.csi-fill.amplitude');
    const phaseFill = this.container.querySelector('.csi-fill.phase');
    const amplitudeValue = this.container.querySelector('.csi-row:first-child .csi-value');
    const phaseValue = this.container.querySelector('.csi-row:last-child .csi-value');
    
    if (!isActive) {
      // Set to zero when no antennas active
      if (amplitudeFill) amplitudeFill.style.width = '0%';
      if (phaseFill) phaseFill.style.width = '0%';
      if (amplitudeValue) amplitudeValue.textContent = '0.00';
      if (phaseValue) phaseValue.textContent = '0.0π';
      return;
    }
    
    // Generate realistic CSI values based on active antennas
    const txCount = activeAntennas.filter(a => a.classList.contains('tx')).length;
    const rxCount = activeAntennas.filter(a => a.classList.contains('rx')).length;
    
    // Amplitude increases with more active antennas
    const baseAmplitude = 0.3 + (txCount * 0.1) + (rxCount * 0.05);
    const amplitude = Math.min(0.95, baseAmplitude + (Math.random() * 0.1 - 0.05));
    
    // Phase varies more with multiple antennas
    const phaseVariation = 0.5 + (activeAntennas.length * 0.1);
    const phase = 0.5 + Math.random() * phaseVariation;
    
    // Update display
    if (amplitudeFill) {
      amplitudeFill.style.width = `${amplitude * 100}%`;
      amplitudeFill.style.transition = 'width 0.5s ease';
    }
    
    if (phaseFill) {
      phaseFill.style.width = `${phase * 50}%`;
      phaseFill.style.transition = 'width 0.5s ease';
    }
    
    if (amplitudeValue) {
      amplitudeValue.textContent = amplitude.toFixed(2);
    }
    
    if (phaseValue) {
      phaseValue.textContent = `${phase.toFixed(1)}π`;
    }
    
    // Update antenna array visualization
    this.updateAntennaArray(activeAntennas);
  }

  // Update antenna array visualization
  updateAntennaArray(activeAntennas) {
    const arrayStatus = this.container.querySelector('.array-status');
    if (!arrayStatus) return;
    
    const txActive = activeAntennas.filter(a => a.classList.contains('tx')).length;
    const rxActive = activeAntennas.filter(a => a.classList.contains('rx')).length;
    
    arrayStatus.innerHTML = `
      <div class="array-info">
        <span class="info-label">Active TX:</span>
        <span class="info-value">${txActive}/3</span>
      </div>
      <div class="array-info">
        <span class="info-label">Active RX:</span>
        <span class="info-value">${rxActive}/6</span>
      </div>
      <div class="array-info">
        <span class="info-label">Signal Quality:</span>
        <span class="info-value">${this.calculateSignalQuality(txActive, rxActive)}%</span>
      </div>
    `;
  }

  // Calculate signal quality based on active antennas
  calculateSignalQuality(txCount, rxCount) {
    if (txCount === 0 || rxCount === 0) return 0;
    
    const txRatio = txCount / 3;
    const rxRatio = rxCount / 6;
    const quality = (txRatio * 0.4 + rxRatio * 0.6) * 100;
    
    return Math.round(quality);
  }

  // Toggle all antennas
  toggleAllAntennas(active) {
    this.antennas.forEach(antenna => {
      antenna.classList.toggle('active', active);
    });
    this.updateCSIDisplay();
  }

  // Reset antenna configuration
  resetAntennas() {
    // Set default configuration (all active)
    this.antennas.forEach(antenna => {
      antenna.classList.add('active');
    });
    this.updateCSIDisplay();
  }

  // Clean up
  dispose() {
    if (this.csiUpdateInterval) {
      clearInterval(this.csiUpdateInterval);
      this.csiUpdateInterval = null;
    }
    
    this.antennas.forEach(antenna => {
      antenna.removeEventListener('click', this.toggleAntenna);
    });
  }
}