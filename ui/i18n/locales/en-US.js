/**
 * @file English (US) Language Pack
 * @description WiFi DensePose Application English Translation
 */

export default {
  meta: {
    title: 'WiFi DensePose: Human Tracking Through Walls',
    description: 'Human Tracking Through Walls Using WiFi Signals'
  },

  header: {
    title: 'WiFi DensePose',
    subtitle: 'Human Tracking Through Walls Using WiFi Signals'
  },

  nav: {
    dashboard: 'Dashboard',
    hardware: 'Hardware',
    demo: 'Live Demo',
    architecture: 'Architecture',
    performance: 'Performance',
    applications: 'Applications',
    sensing: 'Sensing',
    training: 'Training',
    observatory: 'Observatory'
  },

  dashboard: {
    title: 'Revolutionary WiFi-Based Human Pose Detection',
    description: 'AI can track your full-body movement through walls using just WiFi signals. Researchers at Carnegie Mellon have trained a neural network to turn basic WiFi signals into detailed wireframe models of human bodies.',
    
    status: {
      title: 'System Status',
      apiServer: 'API Server',
      hardware: 'Hardware',
      inference: 'Inference',
      streaming: 'Streaming',
      datasource: 'Data Source'
    },

    metrics: {
      title: 'System Metrics',
      cpuUsage: 'CPU Usage',
      memoryUsage: 'Memory Usage',
      diskUsage: 'Disk Usage'
    },

    features: {
      title: 'Features'
    },

    stats: {
      title: 'Live Statistics',
      activePersons: 'Active Persons',
      avgConfidence: 'Avg Confidence',
      totalDetections: 'Total Detections',
      zoneOccupancy: 'Zone Occupancy'
    },

    benefits: {
      throughWalls: {
        title: 'Through Walls',
        description: 'Works through solid barriers with no line of sight required'
      },
      privacy: {
        title: 'Privacy-Preserving',
        description: 'No cameras or visual recording - just WiFi signal analysis'
      },
      realtime: {
        title: 'Real-Time',
        description: 'Maps 24 body regions in real-time at 100Hz sampling rate'
      },
      lowCost: {
        title: 'Low Cost',
        description: 'Built using $30 commercial WiFi hardware'
      }
    },

    systemStats: {
      bodyRegions: 'Body Regions',
      samplingRate: 'Sampling Rate',
      accuracy: 'Accuracy (AP@50)',
      hardwareCost: 'Hardware Cost'
    }
  },

  hardware: {
    title: 'Hardware Configuration',
    
    antenna: {
      title: '3×3 Antenna Array',
      helpText: 'Click antennas to toggle their state',
      transmitters: 'Transmitters (3)',
      receivers: 'Receivers (6)'
    },

    wifi: {
      title: 'WiFi Configuration',
      frequency: 'Frequency',
      frequencyValue: '2.4GHz ± 20MHz',
      subcarriers: 'Subcarriers',
      subcarriersValue: '30',
      samplingRate: 'Sampling Rate',
      samplingRateValue: '100 Hz',
      totalCost: 'Total Cost',
      totalCostValue: '$30'
    },

    csi: {
      title: 'Real-time CSI Data',
      amplitude: 'Amplitude',
      phase: 'Phase'
    }
  },

  demo: {
    title: 'Live Demonstration',
    
    controls: {
      startStream: 'Start Stream',
      stopStream: 'Stop Stream',
      ready: 'Ready'
    },

    signal: {
      title: 'WiFi Signal Analysis',
      signalStrength: 'Signal Strength',
      processingLatency: 'Processing Latency'
    },

    pose: {
      title: 'Human Pose Detection',
      personsDetected: 'Persons Detected',
      confidence: 'Confidence',
      keypoints: 'Keypoints'
    }
  },

  architecture: {
    title: 'System Architecture',
    
    steps: {
      csiInput: {
        title: 'CSI Input',
        description: 'Channel State Information collected from WiFi antenna array'
      },
      phaseSanitization: {
        title: 'Phase Sanitization',
        description: 'Remove hardware-specific noise and normalize signal phase'
      },
      modalityTranslation: {
        title: 'Modality Translation',
        description: 'Convert WiFi signals to visual representation using CNN'
      },
      densePose: {
        title: 'DensePose-RCNN',
        description: 'Extract human pose keypoints and body part segmentation'
      },
      wireframeOutput: {
        title: 'Wireframe Output',
        description: 'Generate final human pose wireframe visualization'
      }
    }
  },

  performance: {
    title: 'Performance Analysis',
    
    wifiBased: {
      title: 'WiFi-based (Same Layout)',
      averagePrecision: 'Average Precision',
      ap50: 'AP@50',
      ap75: 'AP@75'
    },

    imageBased: {
      title: 'Image-based (Reference)',
      averagePrecision: 'Average Precision',
      ap50: 'AP@50',
      ap75: 'AP@75'
    },

    advantages: {
      title: 'Advantages & Limitations',
      pros: {
        title: 'Advantages',
        items: [
          'Through-wall detection',
          'Privacy preserving',
          'Lighting independent',
          'Low cost hardware',
          'Uses existing WiFi'
        ]
      },
      cons: {
        title: 'Limitations',
        items: [
          'Performance drops in different layouts',
          'Requires WiFi-compatible devices',
          'Training requires synchronized data'
        ]
      }
    }
  },

  applications: {
    title: 'Real-World Applications',
    
    elderlyCare: {
      title: 'Elderly Care Monitoring',
      description: 'Monitor elderly individuals for falls or emergencies without invading privacy. Track movement patterns and detect anomalies in daily routines.',
      features: ['Fall Detection', 'Activity Monitoring', 'Emergency Alert']
    },

    homeSecurity: {
      title: 'Home Security Systems',
      description: 'Detect intruders and monitor home security without visible cameras. Track multiple persons and identify suspicious movement patterns.',
      features: ['Intrusion Detection', 'Multi-person Tracking', 'Invisible Monitoring']
    },

    healthcare: {
      title: 'Healthcare Patient Monitoring',
      description: 'Monitor patients in hospitals and care facilities. Track vital signs through movement analysis and detect health emergencies.',
      features: ['Vital Sign Analysis', 'Movement Tracking', 'Health Alerts']
    },

    smartBuilding: {
      title: 'Smart Building Occupancy',
      description: 'Optimize building energy consumption by tracking occupancy patterns. Control lighting, HVAC, and security systems automatically.',
      features: ['Energy Optimization', 'Occupancy Tracking', 'Smart Controls']
    },

    arVr: {
      title: 'AR/VR Applications',
      description: 'Enable full-body tracking for virtual and augmented reality applications without wearing additional sensors or cameras.',
      features: ['Full Body Tracking', 'Sensor-free', 'Immersive Experience']
    },

    implementation: {
      title: 'Implementation Considerations',
      description: 'While WiFi DensePose offers revolutionary capabilities, successful implementation requires careful consideration of environment setup, data privacy regulations, and system calibration for optimal performance.'
    }
  },

  training: {
    title: 'Model Training',
    description: 'Record CSI data, train pose estimation models, and manage .rvf files'
  },

  common: {
    loading: 'Loading...',
    error: 'Error',
    success: 'Success',
    warning: 'Warning',
    info: 'Info',
    confirm: 'Confirm',
    cancel: 'Cancel',
    save: 'Save',
    delete: 'Delete',
    edit: 'Edit',
    add: 'Add',
    search: 'Search',
    noData: 'No data available',
    retry: 'Retry',
    close: 'Close',
    back: 'Back',
    next: 'Next',
    previous: 'Previous',
    submit: 'Submit',
    reset: 'Reset'
  },

  errors: {
    initFailed: 'Failed to initialize application. Please refresh the page.',
    unexpectedError: 'An unexpected error occurred',
    connectionLost: 'Connection lost',
    backendUnavailable: 'Backend unavailable — start sensing-server'
  },

  notifications: {
    mockServerActive: 'Mock server active - testing mode',
    connectedToBackend: 'Connected to Rust sensing server',
    pageHidden: 'Page hidden, pausing updates',
    pageVisible: 'Page visible, resuming updates'
  }
};
