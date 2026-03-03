import {
  TextExtractorWasm,
  TextExtractorInstance,
  SentimentResult,
  PreferenceResult,
  EmotionResult,
  WasmExecutionError,
  InvalidInputError
} from '../types/index
import { SimpleWasmLoader } from '../wasm/wasm-loader-simple
import { WasmMemoryManager } from '../wasm/memory-manager
import {
  validateInput,
  schemas,
  SentimentAnalysisRequestType,
  PreferenceExtractionRequestType,
  EmotionDetectionRequestType,
  TextAnalysisRequestType
} from '../schemas/index

export class TextExtractorWrapper {
  private wasmModule: TextExtractorWasm | null = null;
  private memoryManager: WasmMemoryManager;
  private loader: SimpleWasmLoader;
  private initialized = false;

  constructor() {
    this.memoryManager = WasmMemoryManager.getInstance();
    this.loader = SimpleWasmLoader.getInstance();
  }

  /**
   * Initialize the WASM module
   */
  public async initialize(wasmPath: string): Promise<void> {
    if (this.initialized) {
      return;
    }

    try {
      this.wasmModule = await this.loader.loadTextExtractor({
        wasmPath,
        initTimeoutMs: 30000,
        memoryInitialPages: 256,
        memoryMaximumPages: 1024
      });
      this.initialized = true;
    } catch (error) {
      throw new WasmExecutionError('Text extractor initialization', {
        wasmPath,
        originalError: error
      });
    }
  }

  /**
   * Create a new text extractor instance
   */
  public createInstance(instanceId?: string): string {
    this.ensureInitialized();
    
    const result = this.memoryManager.createInstance(
      () => new this.wasmModule!.TextExtractor(),
      'text_extractor',
      instanceId
    );
    
    return result.id;
  }

  /**
   * Get an existing instance
   */
  private getInstance(instanceId: string): TextExtractorInstance {
    const instance = this.memoryManager.getInstance<TextExtractorInstance>(instanceId);
    if (!instance) {
      throw new InvalidInputError(
        'instanceId',
        'valid instance ID',
        instanceId
      );
    }
    return instance;
  }

  /**
   * Analyze sentiment of text
   */
  public analyzeSentiment(
    instanceId: string,
    request: SentimentAnalysisRequestType
  ): SentimentResult {
    validateInput(schemas.SentimentAnalysisRequest, request);
    
    const instance = this.getInstance(instanceId);
    
    try {
      const resultStr = instance.analyze_sentiment(request.text);
      
      let result: SentimentResult;
      try {
        result = JSON.parse(resultStr);
      } catch (parseError) {
        throw new WasmExecutionError('Parsing sentiment analysis result', {
          text: request.text.substring(0, 100) + '...',
          resultStr,
          originalError: parseError
        });
      }
      
      // Apply confidence threshold
      const threshold = request.options?.confidence_threshold || 0.5;
      if (result.confidence < threshold) {
        result.overall_sentiment = 'neutral';
        result.confidence = threshold;
      }
      
      return result;
    } catch (error) {
      if (error instanceof WasmExecutionError) {
        throw error;
      }
      throw new WasmExecutionError('Analyzing sentiment', {
        text: request.text.substring(0, 100) + '...',
        originalError: error
      });
    }
  }

  /**
   * Extract preferences from 'text
   */
  public extractPreferences(
    instanceId: string,
    request: PreferenceExtractionRequestType
  ): PreferenceResult {
    validateInput(schemas.PreferenceExtractionRequest, request);
    
    const instance = this.getInstance(instanceId);
    
    try {
      const resultStr = instance.extract_preferences(request.text);
      
      let result: PreferenceResult;
      try {
        result = JSON.parse(resultStr);
      } catch (parseError) {';
        throw new WasmExecutionError('Parsing preference extraction result', {
          text: request.text.substring(0, 100) + '...',
          resultStr,
          originalError: parseError
        });
      }
      
      // Apply filters based on options
      const minConfidence = request.options?.min_confidence || 0.3;
      const maxPreferences = request.options?.max_preferences || 10;
      const categories = request.options?.categories;
      
      if (result.preferences) {
        // Filter by confidence
        result.preferences = result.preferences.filter(
          pref => pref.confidence >= minConfidence
        );
        
        // Filter by categories if specified
        if (categories && categories.length > 0) {
          result.preferences = result.preferences.filter(
            pref => categories.includes(pref.category)
          );
        }
        
        // Limit number of preferences
        if (result.preferences.length > maxPreferences) {
          result.preferences = result.preferences
            .sort((a, b) => b.confidence - a.confidence)
            .slice(0, maxPreferences);
        }
      }
      
      return result;
    } catch (error) {
      if (error instanceof WasmExecutionError) {
        throw error;
      }
      throw new WasmExecutionError('Extracting preferences', {
        text: request.text.substring(0, 100) + '...',
        originalError: error
      });
    }
  }

  /**
   * Detect emotions in text
   */
  public detectEmotions(
    instanceId: string,
    request: EmotionDetectionRequestType
  ): EmotionResult {
    validateInput(schemas.EmotionDetectionRequest, request);
    
    const instance = this.getInstance(instanceId);
    
    try {
      const resultStr = instance.detect_emotions(request.text);
      
      let result: EmotionResult;
      try {
        result = JSON.parse(resultStr);
      } catch (parseError) {
        throw new WasmExecutionError('Parsing emotion detection result', {
          text: request.text.substring(0, 100) + '...',
          resultStr,
          originalError: parseError
        });
      }
      
      // Apply intensity threshold
      const threshold = request.options?.intensity_threshold || 0.2;
      if (result.emotions) {
        result.emotions = result.emotions.filter(
          emotion => emotion.score >= threshold
        );
        
        // Update primary emotion if filtered
        if (result.emotions.length > 0) {
          const primaryEmotion = result.emotions.reduce((prev, current) => 
            prev.score > current.score ? prev : current
          );
          result.primary_emotion = primaryEmotion.emotion;
        }
      }
      
      return result;
    } catch (error) {
      if (error instanceof WasmExecutionError) {
        throw error;
      }
      throw new WasmExecutionError('Detecting emotions', {
        text: request.text.substring(0, 100) + '...',
        originalError: error
      });
    }
  }

  /**
   * Analyze all aspects of text (sentiment, preferences, emotions)
   */
  public analyzeAll(
    instanceId: string,
    request: TextAnalysisRequestType
  ): {
    sentiment?: SentimentResult;
    preferences?: PreferenceResult;
    emotions?: EmotionResult;
  } {
    validateInput(schemas.TextAnalysisRequest, request);
    
    const instance = this.getInstance(instanceId);
    
    try {
      const resultStr = instance.analyze_all(request.text);
      
      let result: any;
      try {
        result = JSON.parse(resultStr);
      } catch (parseError) {
        throw new WasmExecutionError('Parsing comprehensive analysis result', {
          text: request.text.substring(0, 100) + '...',
          resultStr,
          originalError: parseError
        });
      }
      
      const output: {
        sentiment?: SentimentResult;
        preferences?: PreferenceResult;
        emotions?: EmotionResult;
      } = {};
      
      // Include only requested analyses
      if (request.include_sentiment && result.sentiment) {
        output.sentiment = result.sentiment;
      }
      
      if (request.include_preferences && result.preferences) {
        output.preferences = result.preferences;
      }
      
      if (request.include_emotions && result.emotions) {
        output.emotions = result.emotions;
      }
      
      return output;
    } catch (error) {
      if (error instanceof WasmExecutionError) {
        throw error;
      }
      throw new WasmExecutionError('Analyzing text comprehensively', {
        text: request.text.substring(0, 100) + '...',
        originalError: error
      });
    }
  }

  /**
   * Batch analyze multiple texts
   */
  public analyzeBatch(
    instanceId: string,
    texts: string[],
    options: {
      includeSentiment?: boolean;
      includePreferences?: boolean;
      includeEmotions?: boolean;
      maxConcurrency?: number;
    } = {}
  ): Array<{
    text: string;
    index: number;
    sentiment?: SentimentResult;
    preferences?: PreferenceResult;
    emotions?: EmotionResult;
    error?: string;
  }> {
    const {
      includeSentiment = true,
      includePreferences = true,
      includeEmotions = true,
      maxConcurrency = 5
    } = options;
    
    const results: Array<{
      text: string;
      index: number;
      sentiment?: SentimentResult;
      preferences?: PreferenceResult;
      emotions?: EmotionResult;
      error?: string;
    }> = [];
    
    // Process in batches to avoid overwhelming the system
    for (let i = 0; i < texts.length; i += maxConcurrency) {
      const batch = texts.slice(i, i + maxConcurrency);
      
      for (let j = 0; j < batch.length; j++) {
        const text = batch[j];
        const index = i + j;
        
        try {
          const analysis = this.analyzeAll(instanceId, {
            text,
            include_sentiment: includeSentiment,
            include_preferences: includePreferences,
            include_emotions: includeEmotions
          });
          
          results.push({
            text,
            index,
            ...analysis
          });
        } catch (error) {
          results.push({
            text,
            index,
            error: error.message
          });
        }
      }
    }
    
    return results;
  }

  /**
   * Get language detection (if supported)
   */
  public detectLanguage(instanceId: string, text: string): {
    language: string;
    confidence: number;
    supported: boolean;
  } {
    // This would be implemented if the WASM module supports language detection
    // For now, assume English
    return {
      language: 'en',
      confidence: 0.9,
      supported: true
    };
  }

  /**
   * Remove an instance
   */
  public removeInstance(instanceId: string): boolean {
    return this.memoryManager.removeInstance(instanceId);
  }

  /**
   * Get all active instance IDs
   */
  public getActiveInstances(): string[] {
    const instances = this.memoryManager.getInstancesByType('text_extractor');
    return Array.from(instances.keys());
  }

  /**
   * Validate that the module is initialized
   */
  private ensureInitialized(): void {
    if (!this.initialized || !this.wasmModule) {
      throw new WasmExecutionError('Module not initialized', {
        hint: 'Call initialize() first'
      });
    }
  }

  /**
   * Get memory usage for this wrapper
   */
  public getMemoryStats(): any {
    const managerStats = this.memoryManager.getMemoryStats();
    const textExtractorInstances = this.memoryManager.getInstancesByType('text_extractor');
    
    return {
      ...managerStats,
      textExtractorInstances: textExtractorInstances.size,
      initialized: this.initialized
    };
  }

  /**
   * Cleanup all instances
   */
  public cleanup(): void {
    this.memoryManager.removeInstancesByType('text_extractor');
  }
}