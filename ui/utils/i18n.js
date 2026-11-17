// Internationalization - EN/PL/PT-BR language support
// Detects browser language, persists choice, translates UI strings

const translations = {
  en: {
    // Navigation
    'nav.dashboard': 'Dashboard',
    'nav.hardware': 'Hardware',
    'nav.demo': 'Live Demo',
    'nav.architecture': 'Architecture',
    'nav.performance': 'Performance',
    'nav.applications': 'Applications',
    'nav.sensing': 'Sensing',
    'nav.training': 'Training',
    'nav.poseFusion': 'Pose Fusion',
    'nav.observatory': 'Observatory',

    // Dashboard
    'dashboard.title': 'Revolutionary WiFi-Based Human Pose Detection',
    'dashboard.subtitle': 'Human Tracking Through Walls Using WiFi Signals',
    'dashboard.description': 'AI can track your full-body movement through walls using just WiFi signals. Researchers at Carnegie Mellon have trained a neural network to turn basic WiFi signals into detailed wireframe models of human bodies.',
    'dashboard.status': 'System Status',
    'dashboard.metrics': 'System Metrics',
    'dashboard.features': 'Features',
    'dashboard.liveStats': 'Live Statistics',
    'dashboard.activePersons': 'Active Persons',
    'dashboard.avgConfidence': 'Avg Confidence',
    'dashboard.totalDetections': 'Total Detections',
    'dashboard.zoneOccupancy': 'Zone Occupancy',

    // Status
    'status.apiServer': 'API Server',
    'status.hardware': 'Hardware',
    'status.inference': 'Inference',
    'status.streaming': 'Streaming',
    'status.dataSource': 'Data Source',

    // Metrics
    'metrics.cpu': 'CPU Usage',
    'metrics.memory': 'Memory Usage',
    'metrics.disk': 'Disk Usage',

    // Benefits
    'benefit.throughWalls': 'Through Walls',
    'benefit.throughWallsDesc': 'Works through solid barriers with no line of sight required',
    'benefit.privacy': 'Privacy-Preserving',
    'benefit.privacyDesc': 'No cameras or visual recording - just WiFi signal analysis',
    'benefit.realtime': 'Real-Time',
    'benefit.realtimeDesc': 'Maps 24 body regions in real-time at 100Hz sampling rate',
    'benefit.lowCost': 'Low Cost',
    'benefit.lowCostDesc': 'Built using $30 commercial WiFi hardware',

    // Stats
    'stat.bodyRegions': 'Body Regions',
    'stat.samplingRate': 'Sampling Rate',
    'stat.accuracy': 'Accuracy (AP@50)',
    'stat.hardwareCost': 'Hardware Cost',

    // Hardware
    'hardware.title': 'Hardware Configuration',
    'hardware.antennaArray': '3×3 Antenna Array',
    'hardware.toggleHint': 'Click antennas to toggle their state',
    'hardware.transmitters': 'Transmitters (3)',
    'hardware.receivers': 'Receivers (6)',
    'hardware.wifiConfig': 'WiFi Configuration',
    'hardware.frequency': 'Frequency',
    'hardware.subcarriers': 'Subcarriers',
    'hardware.samplingRate': 'Sampling Rate',
    'hardware.totalCost': 'Total Cost',
    'hardware.realtimeCsi': 'Real-time CSI Data',
    'hardware.amplitude': 'Amplitude:',
    'hardware.phase': 'Phase:',

    // Demo
    'demo.title': 'Live Demonstration',
    'demo.startStream': 'Start Stream',
    'demo.stopStream': 'Stop Stream',
    'demo.ready': 'Ready',
    'demo.signalAnalysis': 'WiFi Signal Analysis',
    'demo.signalStrength': 'Signal Strength:',
    'demo.processingLatency': 'Processing Latency:',
    'demo.poseDetection': 'Human Pose Detection',
    'demo.personsDetected': 'Persons Detected:',
    'demo.confidence': 'Confidence:',
    'demo.keypoints': 'Keypoints:',

    // Architecture
    'architecture.title': 'System Architecture',
    'architecture.csiInput': 'CSI Input',
    'architecture.csiInputDesc': 'Channel State Information collected from WiFi antenna array',
    'architecture.phaseSanitization': 'Phase Sanitization',
    'architecture.phaseSanitizationDesc': 'Remove hardware-specific noise and normalize signal phase',
    'architecture.modalityTranslation': 'Modality Translation',
    'architecture.modalityTranslationDesc': 'Convert WiFi signals to visual representation using CNN',
    'architecture.densepose': 'DensePose-RCNN',
    'architecture.denseposeDesc': 'Extract human pose keypoints and body part segmentation',
    'architecture.wireframeOutput': 'Wireframe Output',
    'architecture.wireframeOutputDesc': 'Generate final human pose wireframe visualization',

    // Performance
    'performance.title': 'Performance Analysis',
    'performance.wifiSameLayout': 'WiFi-based (Same Layout)',
    'performance.imageReference': 'Image-based (Reference)',
    'performance.avgPrecision': 'Average Precision:',
    'performance.advantagesLimitations': 'Advantages & Limitations',
    'performance.advantages': 'Advantages',
    'performance.limitations': 'Limitations',
    'performance.proThroughWall': 'Through-wall detection',
    'performance.proPrivacy': 'Privacy preserving',
    'performance.proLighting': 'Lighting independent',
    'performance.proLowCost': 'Low cost hardware',
    'performance.proExistingWifi': 'Uses existing WiFi',
    'performance.conLayout': 'Performance drops in different layouts',
    'performance.conDevices': 'Requires WiFi-compatible devices',
    'performance.conTraining': 'Training requires synchronized data',

    // Applications
    'applications.title': 'Real-World Applications',
    'applications.elderlyCare': 'Elderly Care Monitoring',
    'applications.elderlyCareDesc': 'Monitor elderly individuals for falls or emergencies without invading privacy. Track movement patterns and detect anomalies in daily routines.',
    'applications.homeSecurity': 'Home Security Systems',
    'applications.homeSecurityDesc': 'Detect intruders and monitor home security without visible cameras. Track multiple persons and identify suspicious movement patterns.',
    'applications.healthcare': 'Healthcare Patient Monitoring',
    'applications.healthcareDesc': 'Monitor patients in hospitals and care facilities. Track vital signs through movement analysis and detect health emergencies.',
    'applications.smartBuilding': 'Smart Building Occupancy',
    'applications.smartBuildingDesc': 'Optimize building energy consumption by tracking occupancy patterns. Control lighting, HVAC, and security systems automatically.',
    'applications.arvr': 'AR/VR Applications',
    'applications.arvrDesc': 'Enable full-body tracking for virtual and augmented reality applications without wearing additional sensors or cameras.',
    'applications.fallDetection': 'Fall Detection',
    'applications.activityMonitoring': 'Activity Monitoring',
    'applications.emergencyAlert': 'Emergency Alert',
    'applications.intrusionDetection': 'Intrusion Detection',
    'applications.multiPersonTracking': 'Multi-person Tracking',
    'applications.invisibleMonitoring': 'Invisible Monitoring',
    'applications.vitalSignAnalysis': 'Vital Sign Analysis',
    'applications.movementTracking': 'Movement Tracking',
    'applications.healthAlerts': 'Health Alerts',
    'applications.energyOptimization': 'Energy Optimization',
    'applications.occupancyTracking': 'Occupancy Tracking',
    'applications.smartControls': 'Smart Controls',
    'applications.fullBodyTracking': 'Full Body Tracking',
    'applications.sensorFree': 'Sensor-free',
    'applications.immersiveExperience': 'Immersive Experience',
    'applications.considerations': 'Implementation Considerations',
    'applications.considerationsDesc': 'While WiFi DensePose offers revolutionary capabilities, successful implementation requires careful consideration of environment setup, data privacy regulations, and system calibration for optimal performance.',

    // Training
    'training.title': 'Model Training',
    'training.description': 'Record CSI data, train pose estimation models, and manage .rvf files',

    // Actions
    'action.startDetection': 'Start Detection',
    'action.stopDetection': 'Stop Detection',
    'action.toggleTheme': 'Toggle theme',
    'action.exportData': 'Export data',
    'action.screenshot': 'Take screenshot',

    // Connection
    'conn.connected': 'Connected',
    'conn.connecting': 'Connecting...',
    'conn.offline': 'Offline',
    'conn.reconnecting': 'Reconnecting...',
    'conn.live': 'Live',
    'conn.simulated': 'Simulated',

    // Misc
    'misc.loading': 'Loading...',
    'misc.error': 'An error occurred',
    'misc.noData': 'No data available',
    'misc.close': 'Close',
    'misc.cancel': 'Cancel',
    'misc.confirm': 'Confirm',
    'misc.settings': 'Settings',
    'misc.language': 'Language',
    'misc.skipToContent': 'Skip to main content',

    // Command palette
    'command.category.navigation': 'Navigation',
    'command.category.actions': 'Actions',
    'command.goTo': 'Go to {label}',
    'command.openPoseFusion': 'Open Pose Fusion',
    'command.openObservatory': 'Open Observatory',
    'command.toggleTheme': 'Toggle Dark/Light Theme',
    'command.togglePerfMonitor': 'Toggle Performance Monitor',
    'command.toggleActivityLog': 'Toggle Activity Log',
    'command.exportSensorData': 'Export Sensor Data',
    'command.toggleFullscreen': 'Toggle Fullscreen',
    'command.showShortcuts': 'Show Keyboard Shortcuts',
    'command.paletteLabel': 'Command palette',
    'command.searchPlaceholder': 'Type a command...',
    'command.searchAria': 'Search commands',
    'command.resultsAria': 'Commands',
    'command.noMatches': 'No matching commands',
    'command.footerNavigate': 'navigate',
    'command.footerExecute': 'execute',
    'command.footerClose': 'close'
  },

  pl: {
    // Navigation
    'nav.dashboard': 'Panel',
    'nav.hardware': 'Sprzet',
    'nav.demo': 'Demo na zywo',
    'nav.architecture': 'Architektura',
    'nav.performance': 'Wydajnosc',
    'nav.applications': 'Aplikacje',
    'nav.sensing': 'Czujniki',
    'nav.training': 'Trening',

    // Dashboard
    'dashboard.title': 'Rewolucyjne wykrywanie pozy czlowieka przez WiFi',
    'dashboard.subtitle': 'Sledzenie ludzi przez sciany za pomoca sygnalow WiFi',
    'dashboard.description': 'AI moze sledzic ruchy calego ciala przez sciany uzywajac jedynie sygnalow WiFi. Badacze z Carnegie Mellon wytrenowali siec neuronowa do zamiany sygnalow WiFi w szczegolowe modele szkieletowe.',
    'dashboard.status': 'Status systemu',
    'dashboard.metrics': 'Metryki systemu',
    'dashboard.features': 'Funkcje',
    'dashboard.liveStats': 'Statystyki na zywo',
    'dashboard.activePersons': 'Aktywne osoby',
    'dashboard.avgConfidence': 'Srednia pewnosc',
    'dashboard.totalDetections': 'Laczne detekcje',
    'dashboard.zoneOccupancy': 'Zajecie stref',

    // Status
    'status.apiServer': 'Serwer API',
    'status.hardware': 'Sprzet',
    'status.inference': 'Wnioskowanie',
    'status.streaming': 'Streaming',
    'status.dataSource': 'Zrodlo danych',

    // Metrics
    'metrics.cpu': 'Uzycie CPU',
    'metrics.memory': 'Uzycie pamieci',
    'metrics.disk': 'Uzycie dysku',

    // Benefits
    'benefit.throughWalls': 'Przez sciany',
    'benefit.throughWallsDesc': 'Dziala przez przeszkody stale bez linii wzroku',
    'benefit.privacy': 'Ochrona prywatnosci',
    'benefit.privacyDesc': 'Brak kamer i nagrywania - tylko analiza sygnalow WiFi',
    'benefit.realtime': 'Czas rzeczywisty',
    'benefit.realtimeDesc': 'Mapuje 24 regiony ciala w czasie rzeczywistym przy 100Hz',
    'benefit.lowCost': 'Niski koszt',
    'benefit.lowCostDesc': 'Zbudowany z komercyjnego sprzetu WiFi za $30',

    // Stats
    'stat.bodyRegions': 'Regiony ciala',
    'stat.samplingRate': 'Czestotliwosc',
    'stat.accuracy': 'Dokladnosc (AP@50)',
    'stat.hardwareCost': 'Koszt sprzetu',

    // Actions
    'action.startDetection': 'Rozpocznij detekcje',
    'action.stopDetection': 'Zatrzymaj detekcje',
    'action.toggleTheme': 'Zmien motyw',
    'action.exportData': 'Eksportuj dane',
    'action.screenshot': 'Zrob zrzut ekranu',

    // Connection
    'conn.connected': 'Polaczono',
    'conn.connecting': 'Laczenie...',
    'conn.offline': 'Offline',
    'conn.reconnecting': 'Ponowne laczenie...',
    'conn.live': 'Na zywo',
    'conn.simulated': 'Symulacja',

    // Misc
    'misc.loading': 'Ladowanie...',
    'misc.error': 'Wystapil blad',
    'misc.noData': 'Brak danych',
    'misc.close': 'Zamknij',
    'misc.cancel': 'Anuluj',
    'misc.confirm': 'Potwierdz',
    'misc.settings': 'Ustawienia',
    'misc.language': 'Jezyk'
  },

  'pt-BR': {
    // Navigation
    'nav.dashboard': 'Painel',
    'nav.hardware': 'Hardware',
    'nav.demo': 'Demo ao Vivo',
    'nav.architecture': 'Arquitetura',
    'nav.performance': 'Desempenho',
    'nav.applications': 'Aplicações',
    'nav.sensing': 'Sensoriamento',
    'nav.training': 'Treinamento',
    'nav.poseFusion': 'Fusão de Pose',
    'nav.observatory': 'Observatório',

    // Dashboard
    'dashboard.title': 'Detecção de Pose Humana por WiFi',
    'dashboard.subtitle': 'Rastreamento humano através de paredes usando sinais WiFi',
    'dashboard.description': 'A IA consegue acompanhar o movimento do corpo inteiro através de paredes usando apenas sinais WiFi. Pesquisadores da Carnegie Mellon treinaram uma rede neural para transformar sinais WiFi básicos em modelos detalhados do corpo humano.',
    'dashboard.status': 'Status do Sistema',
    'dashboard.metrics': 'Métricas do Sistema',
    'dashboard.features': 'Recursos',
    'dashboard.liveStats': 'Estatísticas ao Vivo',
    'dashboard.activePersons': 'Pessoas Ativas',
    'dashboard.avgConfidence': 'Confiança Média',
    'dashboard.totalDetections': 'Detecções Totais',
    'dashboard.zoneOccupancy': 'Ocupação por Zona',

    // Status
    'status.apiServer': 'Servidor API',
    'status.hardware': 'Hardware',
    'status.inference': 'Inferência',
    'status.streaming': 'Streaming',
    'status.dataSource': 'Fonte de Dados',

    // Metrics
    'metrics.cpu': 'Uso de CPU',
    'metrics.memory': 'Uso de Memória',
    'metrics.disk': 'Uso de Disco',

    // Benefits
    'benefit.throughWalls': 'Através de Paredes',
    'benefit.throughWallsDesc': 'Funciona através de barreiras sólidas sem linha de visão',
    'benefit.privacy': 'Preserva a Privacidade',
    'benefit.privacyDesc': 'Sem câmeras ou gravação visual, apenas análise de sinais WiFi',
    'benefit.realtime': 'Tempo Real',
    'benefit.realtimeDesc': 'Mapeia 24 regiões do corpo em tempo real a 100 Hz',
    'benefit.lowCost': 'Baixo Custo',
    'benefit.lowCostDesc': 'Construído com hardware WiFi comercial de US$ 30',

    // Stats
    'stat.bodyRegions': 'Regiões do Corpo',
    'stat.samplingRate': 'Taxa de Amostragem',
    'stat.accuracy': 'Precisão (AP@50)',
    'stat.hardwareCost': 'Custo do Hardware',

    // Hardware
    'hardware.title': 'Configuração de Hardware',
    'hardware.antennaArray': 'Arranjo de Antenas 3x3',
    'hardware.toggleHint': 'Clique nas antenas para alternar o estado',
    'hardware.transmitters': 'Transmissores (3)',
    'hardware.receivers': 'Receptores (6)',
    'hardware.wifiConfig': 'Configuração WiFi',
    'hardware.frequency': 'Frequência',
    'hardware.subcarriers': 'Subportadoras',
    'hardware.samplingRate': 'Taxa de Amostragem',
    'hardware.totalCost': 'Custo Total',
    'hardware.realtimeCsi': 'Dados CSI em Tempo Real',
    'hardware.amplitude': 'Amplitude:',
    'hardware.phase': 'Fase:',

    // Demo
    'demo.title': 'Demonstração ao Vivo',
    'demo.startStream': 'Iniciar Stream',
    'demo.stopStream': 'Parar Stream',
    'demo.ready': 'Pronto',
    'demo.signalAnalysis': 'Análise do Sinal WiFi',
    'demo.signalStrength': 'Força do Sinal:',
    'demo.processingLatency': 'Latência de Processamento:',
    'demo.poseDetection': 'Detecção de Pose Humana',
    'demo.personsDetected': 'Pessoas Detectadas:',
    'demo.confidence': 'Confiança:',
    'demo.keypoints': 'Pontos-chave:',

    // Architecture
    'architecture.title': 'Arquitetura do Sistema',
    'architecture.csiInput': 'Entrada CSI',
    'architecture.csiInputDesc': 'Informações de estado do canal coletadas do arranjo de antenas WiFi',
    'architecture.phaseSanitization': 'Sanitização de Fase',
    'architecture.phaseSanitizationDesc': 'Remove ruído específico do hardware e normaliza a fase do sinal',
    'architecture.modalityTranslation': 'Tradução de Modalidade',
    'architecture.modalityTranslationDesc': 'Converte sinais WiFi em representação visual usando CNN',
    'architecture.densepose': 'DensePose-RCNN',
    'architecture.denseposeDesc': 'Extrai pontos-chave da pose humana e segmentação de partes do corpo',
    'architecture.wireframeOutput': 'Saída em Wireframe',
    'architecture.wireframeOutputDesc': 'Gera a visualização final da pose humana em wireframe',

    // Performance
    'performance.title': 'Análise de Desempenho',
    'performance.wifiSameLayout': 'Baseado em WiFi (mesmo layout)',
    'performance.imageReference': 'Baseado em imagem (referência)',
    'performance.avgPrecision': 'Precisão média:',
    'performance.advantagesLimitations': 'Vantagens e Limitações',
    'performance.advantages': 'Vantagens',
    'performance.limitations': 'Limitações',
    'performance.proThroughWall': 'Detecção através de paredes',
    'performance.proPrivacy': 'Preserva a privacidade',
    'performance.proLighting': 'Independente da iluminação',
    'performance.proLowCost': 'Hardware de baixo custo',
    'performance.proExistingWifi': 'Usa WiFi existente',
    'performance.conLayout': 'O desempenho cai em layouts diferentes',
    'performance.conDevices': 'Exige dispositivos compatíveis com WiFi',
    'performance.conTraining': 'O treinamento exige dados sincronizados',

    // Applications
    'applications.title': 'Aplicações no Mundo Real',
    'applications.elderlyCare': 'Monitoramento de Idosos',
    'applications.elderlyCareDesc': 'Monitore idosos para quedas ou emergências sem invadir a privacidade. Acompanhe padrões de movimento e detecte anomalias na rotina diária.',
    'applications.homeSecurity': 'Sistemas de Segurança Residencial',
    'applications.homeSecurityDesc': 'Detecte intrusos e monitore a segurança residencial sem câmeras visíveis. Acompanhe várias pessoas e identifique padrões de movimento suspeitos.',
    'applications.healthcare': 'Monitoramento de Pacientes',
    'applications.healthcareDesc': 'Monitore pacientes em hospitais e instituições de cuidado. Acompanhe sinais vitais por análise de movimento e detecte emergências de saúde.',
    'applications.smartBuilding': 'Ocupação em Prédios Inteligentes',
    'applications.smartBuildingDesc': 'Otimize o consumo de energia acompanhando padrões de ocupação. Controle iluminação, HVAC e sistemas de segurança automaticamente.',
    'applications.arvr': 'Aplicações AR/VR',
    'applications.arvrDesc': 'Permite rastreamento de corpo inteiro para realidade virtual e aumentada sem sensores adicionais ou câmeras.',
    'applications.fallDetection': 'Detecção de Quedas',
    'applications.activityMonitoring': 'Monitoramento de Atividade',
    'applications.emergencyAlert': 'Alerta de Emergência',
    'applications.intrusionDetection': 'Detecção de Intrusão',
    'applications.multiPersonTracking': 'Rastreamento de Múltiplas Pessoas',
    'applications.invisibleMonitoring': 'Monitoramento Invisível',
    'applications.vitalSignAnalysis': 'Análise de Sinais Vitais',
    'applications.movementTracking': 'Rastreamento de Movimento',
    'applications.healthAlerts': 'Alertas de Saúde',
    'applications.energyOptimization': 'Otimização de Energia',
    'applications.occupancyTracking': 'Rastreamento de Ocupação',
    'applications.smartControls': 'Controles Inteligentes',
    'applications.fullBodyTracking': 'Rastreamento de Corpo Inteiro',
    'applications.sensorFree': 'Sem Sensores',
    'applications.immersiveExperience': 'Experiência Imersiva',
    'applications.considerations': 'Considerações de Implementação',
    'applications.considerationsDesc': 'Embora o WiFi DensePose ofereça recursos avançados, uma implementação bem-sucedida exige cuidado com a configuração do ambiente, normas de privacidade de dados e calibração do sistema para desempenho ideal.',

    // Training
    'training.title': 'Treinamento do Modelo',
    'training.description': 'Grave dados CSI, treine modelos de estimativa de pose e gerencie arquivos .rvf',

    // Actions
    'action.startDetection': 'Iniciar Detecção',
    'action.stopDetection': 'Parar Detecção',
    'action.toggleTheme': 'Alternar tema',
    'action.exportData': 'Exportar dados',
    'action.screenshot': 'Capturar tela',

    // Connection
    'conn.connected': 'Conectado',
    'conn.connecting': 'Conectando...',
    'conn.offline': 'Offline',
    'conn.reconnecting': 'Reconectando...',
    'conn.live': 'Ao vivo',
    'conn.simulated': 'Simulado',

    // Misc
    'misc.loading': 'Carregando...',
    'misc.error': 'Ocorreu um erro',
    'misc.noData': 'Nenhum dado disponível',
    'misc.close': 'Fechar',
    'misc.cancel': 'Cancelar',
    'misc.confirm': 'Confirmar',
    'misc.settings': 'Configurações',
    'misc.language': 'Idioma',
    'misc.skipToContent': 'Pular para o conteúdo principal',

    // Command palette
    'command.category.navigation': 'Navegação',
    'command.category.actions': 'Ações',
    'command.goTo': 'Ir para {label}',
    'command.openPoseFusion': 'Abrir Fusão de Pose',
    'command.openObservatory': 'Abrir Observatório',
    'command.toggleTheme': 'Alternar Tema Claro/Escuro',
    'command.togglePerfMonitor': 'Alternar Monitor de Desempenho',
    'command.toggleActivityLog': 'Alternar Log de Atividades',
    'command.exportSensorData': 'Exportar Dados dos Sensores',
    'command.toggleFullscreen': 'Alternar Tela Cheia',
    'command.showShortcuts': 'Mostrar Atalhos de Teclado',
    'command.paletteLabel': 'Paleta de comandos',
    'command.searchPlaceholder': 'Digite um comando...',
    'command.searchAria': 'Buscar comandos',
    'command.resultsAria': 'Comandos',
    'command.noMatches': 'Nenhum comando encontrado',
    'command.footerNavigate': 'navegar',
    'command.footerExecute': 'executar',
    'command.footerClose': 'fechar'
  }
};

export class I18n {
  constructor() {
    this.locale = this.getSavedLocale() || this.detectLocale();
    this.listeners = [];
  }

  init() {
    this.createSelector();
    this.applyTranslations();
  }

  detectLocale() {
    const lang = navigator.language?.toLowerCase() || 'en';
    if (lang.startsWith('pt')) return 'pt-BR';
    if (lang.startsWith('pl')) return 'pl';
    return 'en';
  }

  getSavedLocale() {
    try { return localStorage.getItem('ruview-locale'); }
    catch { return null; }
  }

  saveLocale(locale) {
    try { localStorage.setItem('ruview-locale', locale); }
    catch { /* noop */ }
  }

  t(key) {
    const dict = translations[this.locale] || translations.en;
    return dict[key] || translations.en[key] || key;
  }

  format(key, values = {}) {
    return this.t(key).replace(/\{(\w+)\}/g, (_, name) => values[name] ?? '');
  }

  setLocale(locale) {
    if (!translations[locale]) return;
    this.locale = locale;
    this.saveLocale(locale);
    document.documentElement.setAttribute('lang', locale);
    this.applyTranslations();
    this.listeners.forEach(cb => { try { cb(locale); } catch { /* noop */ } });
  }

  onLocaleChange(callback) {
    this.listeners.push(callback);
    return () => {
      const i = this.listeners.indexOf(callback);
      if (i > -1) this.listeners.splice(i, 1);
    };
  }

  applyTranslations() {
    // Translate elements with data-i18n attribute
    document.querySelectorAll('[data-i18n]').forEach(el => {
      const key = el.getAttribute('data-i18n');
      el.textContent = this.t(key);
    });

    // Translate placeholders
    document.querySelectorAll('[data-i18n-placeholder]').forEach(el => {
      const key = el.getAttribute('data-i18n-placeholder');
      el.placeholder = this.t(key);
    });

    // Translate aria-labels
    document.querySelectorAll('[data-i18n-aria]').forEach(el => {
      const key = el.getAttribute('data-i18n-aria');
      el.setAttribute('aria-label', this.t(key));
    });

    // Update language selector
    const selector = document.getElementById('lang-selector');
    if (selector) selector.value = this.locale;
  }

  createSelector() {
    const wrapper = document.createElement('div');
    wrapper.className = 'lang-selector-wrap';
    wrapper.innerHTML = `
      <select id="lang-selector" class="lang-selector" aria-label="Language">
        <option value="en">EN</option>
        <option value="pt-BR">PT-BR</option>
        <option value="pl">PL</option>
      </select>
    `;

    const select = wrapper.querySelector('select');
    select.value = this.locale;
    select.addEventListener('change', () => this.setLocale(select.value));

    const headerInfo = document.querySelector('.header-info');
    if (headerInfo) {
      headerInfo.appendChild(wrapper);
    }
  }

  getAvailableLocales() {
    return Object.keys(translations);
  }

  dispose() {
    this.listeners = [];
  }
}

export const i18n = new I18n();
