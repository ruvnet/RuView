/**
 * @file 中文简体语言包
 * @description WiFi DensePose 应用中文简体翻译
 */

export default {
  meta: {
    title: 'WiFi DensePose: 穿墙人体追踪',
    description: '使用 WiFi 信号进行穿墙人体追踪'
  },

  header: {
    title: 'WiFi DensePose',
    subtitle: '使用 WiFi 信号进行穿墙人体追踪'
  },

  nav: {
    dashboard: '仪表盘',
    hardware: '硬件配置',
    demo: '实时演示',
    architecture: '系统架构',
    performance: '性能分析',
    applications: '应用场景',
    sensing: '感知',
    training: '模型训练',
    observatory: '观测站'
  },

  dashboard: {
    title: '革命性的 WiFi 人体姿态检测',
    description: 'AI 可以仅使用 WiFi 信号穿透墙壁追踪您的全身动作。卡内基梅隆大学的研究人员训练了一个神经网络，可以将基本的 WiFi 信号转换为详细的人体线框模型。',
    
    status: {
      title: '系统状态',
      apiServer: 'API 服务器',
      hardware: '硬件',
      inference: '推理引擎',
      streaming: '数据流',
      datasource: '数据源'
    },

    metrics: {
      title: '系统指标',
      cpuUsage: 'CPU 使用率',
      memoryUsage: '内存使用率',
      diskUsage: '磁盘使用率'
    },

    features: {
      title: '功能特性'
    },

    stats: {
      title: '实时统计',
      activePersons: '活动人数',
      avgConfidence: '平均置信度',
      totalDetections: '总检测次数',
      zoneOccupancy: '区域占用'
    },

    benefits: {
      throughWalls: {
        title: '穿墙检测',
        description: '无需视线，可穿透实体障碍物工作'
      },
      privacy: {
        title: '隐私保护',
        description: '无需摄像头或视频录制，仅分析 WiFi 信号'
      },
      realtime: {
        title: '实时处理',
        description: '以 100Hz 采样率实时映射 24 个身体区域'
      },
      lowCost: {
        title: '低成本',
        description: '使用价值 30 美元的商用 WiFi 硬件构建'
      }
    },

    systemStats: {
      bodyRegions: '身体区域',
      samplingRate: '采样率',
      accuracy: '准确率 (AP@50)',
      hardwareCost: '硬件成本'
    }
  },

  hardware: {
    title: '硬件配置',
    
    antenna: {
      title: '3×3 天线阵列',
      helpText: '点击天线切换其状态',
      transmitters: '发射器 (3)',
      receivers: '接收器 (6)'
    },

    wifi: {
      title: 'WiFi 配置',
      frequency: '频率',
      frequencyValue: '2.4GHz ± 20MHz',
      subcarriers: '子载波',
      subcarriersValue: '30',
      samplingRate: '采样率',
      samplingRateValue: '100 Hz',
      totalCost: '总成本',
      totalCostValue: '$30'
    },

    csi: {
      title: '实时 CSI 数据',
      amplitude: '幅度',
      phase: '相位'
    }
  },

  demo: {
    title: '实时演示',
    
    controls: {
      startStream: '开始流',
      stopStream: '停止流',
      ready: '就绪'
    },

    signal: {
      title: 'WiFi 信号分析',
      signalStrength: '信号强度',
      processingLatency: '处理延迟'
    },

    pose: {
      title: '人体姿态检测',
      personsDetected: '检测到的人数',
      confidence: '置信度',
      keypoints: '关键点'
    }
  },

  architecture: {
    title: '系统架构',
    
    steps: {
      csiInput: {
        title: 'CSI 输入',
        description: '从 WiFi 天线阵列收集信道状态信息'
      },
      phaseSanitization: {
        title: '相位净化',
        description: '去除硬件特定噪声并归一化信号相位'
      },
      modalityTranslation: {
        title: '模态转换',
        description: '使用 CNN 将 WiFi 信号转换为视觉表示'
      },
      densePose: {
        title: 'DensePose-RCNN',
        description: '提取人体姿态关键点和身体部位分割'
      },
      wireframeOutput: {
        title: '线框输出',
        description: '生成最终的人体姿态线框可视化'
      }
    }
  },

  performance: {
    title: '性能分析',
    
    wifiBased: {
      title: '基于 WiFi (相同布局)',
      averagePrecision: '平均精度',
      ap50: 'AP@50',
      ap75: 'AP@75'
    },

    imageBased: {
      title: '基于图像 (参考)',
      averagePrecision: '平均精度',
      ap50: 'AP@50',
      ap75: 'AP@75'
    },

    advantages: {
      title: '优势与局限',
      pros: {
        title: '优势',
        items: [
          '穿墙检测',
          '隐私保护',
          '不受光照影响',
          '低成本硬件',
          '利用现有 WiFi'
        ]
      },
      cons: {
        title: '局限',
        items: [
          '不同布局下性能下降',
          '需要兼容 WiFi 的设备',
          '训练需要同步数据'
        ]
      }
    }
  },

  applications: {
    title: '实际应用场景',
    
    elderlyCare: {
      title: '老年人监护',
      description: '在不侵犯隐私的情况下监测老年人的跌倒或紧急情况。追踪活动模式并检测日常行为异常。',
      features: ['跌倒检测', '活动监测', '紧急报警']
    },

    homeSecurity: {
      title: '家庭安防系统',
      description: '无需可见摄像头即可检测入侵者并监控家庭安全。追踪多人并识别可疑行为模式。',
      features: ['入侵检测', '多人追踪', '隐形监控']
    },

    healthcare: {
      title: '医疗患者监护',
      description: '在医院和护理机构监测患者。通过动作分析追踪生命体征并检测健康紧急情况。',
      features: ['生命体征分析', '动作追踪', '健康警报']
    },

    smartBuilding: {
      title: '智能建筑占用',
      description: '通过追踪占用模式优化建筑能耗。自动控制照明、暖通空调和安防系统。',
      features: ['节能优化', '占用追踪', '智能控制']
    },

    arVr: {
      title: 'AR/VR 应用',
      description: '为虚拟和增强现实应用实现全身追踪，无需佩戴额外传感器或摄像头。',
      features: ['全身追踪', '无传感器', '沉浸体验']
    },

    implementation: {
      title: '实施注意事项',
      description: '虽然 WiFi DensePose 提供了革命性的能力，但成功实施需要仔细考虑环境设置、数据隐私法规和系统校准以获得最佳性能。'
    }
  },

  training: {
    title: '模型训练',
    description: '记录 CSI 数据、训练姿态估计模型并管理 .rvf 文件'
  },

  common: {
    loading: '加载中...',
    error: '错误',
    success: '成功',
    warning: '警告',
    info: '信息',
    confirm: '确认',
    cancel: '取消',
    save: '保存',
    delete: '删除',
    edit: '编辑',
    add: '添加',
    search: '搜索',
    noData: '暂无数据',
    retry: '重试',
    close: '关闭',
    back: '返回',
    next: '下一步',
    previous: '上一步',
    submit: '提交',
    reset: '重置'
  },

  errors: {
    initFailed: '应用初始化失败，请刷新页面。',
    unexpectedError: '发生意外错误',
    connectionLost: '连接已断开',
    backendUnavailable: '后端不可用 — 请启动 sensing-server'
  },

  notifications: {
    mockServerActive: '模拟服务器已激活 - 测试模式',
    connectedToBackend: '已连接到 Rust 感知服务器',
    pageHidden: '页面隐藏，暂停更新',
    pageVisible: '页面可见，恢复更新'
  }
};
