/**
 * @file i18n 核心模块 - 提供国际化语言支持
 * @description 轻量级国际化解决方案，支持多语言切换、参数插值、嵌套翻译
 */

const DEFAULT_LOCALE = 'en-US';
const STORAGE_KEY = 'wifi-densepose-locale';

class I18n {
  constructor() {
    this.currentLocale = DEFAULT_LOCALE;
    this.translations = new Map();
    this.fallbackLocale = DEFAULT_LOCALE;
    this.observers = new Set();
  }

  /**
   * 注册语言包
   * @param {string} locale - 语言代码 (如 'zh-CN', 'en-US')
   * @param {Object} messages - 翻译消息对象
   */
  register(locale, messages) {
    if (!locale || typeof locale !== 'string') {
      console.error('[I18n] Invalid locale:', locale);
      return;
    }
    this.translations.set(locale, messages);
    console.log(`[I18n] Registered locale: ${locale}`);
  }

  /**
   * 设置当前语言
   * @param {string} locale - 语言代码
   */
  setLocale(locale) {
    if (!this.translations.has(locale)) {
      console.warn(`[I18n] Locale "${locale}" not registered, using fallback`);
      locale = this.fallbackLocale;
    }
    
    const oldLocale = this.currentLocale;
    this.currentLocale = locale;
    
    try {
      localStorage.setItem(STORAGE_KEY, locale);
    } catch (e) {
      console.warn('[I18n] Failed to save locale to localStorage');
    }
    
    document.documentElement.lang = locale.split('-')[0];
    
    this.observers.forEach(callback => {
      try {
        callback(locale, oldLocale);
      } catch (e) {
        console.error('[I18n] Observer callback error:', e);
      }
    });
    
    console.log(`[I18n] Locale changed: ${oldLocale} -> ${locale}`);
  }

  /**
   * 获取当前语言
   * @returns {string} 当前语言代码
   */
  getLocale() {
    return this.currentLocale;
  }

  /**
   * 获取所有已注册的语言列表
   * @returns {string[]} 语言代码数组
   */
  getAvailableLocales() {
    return Array.from(this.translations.keys());
  }

  /**
   * 初始化语言设置
   * @returns {string} 检测到的语言代码
   */
  init() {
    let savedLocale = null;
    
    try {
      savedLocale = localStorage.getItem(STORAGE_KEY);
    } catch (e) {
      console.warn('[I18n] Failed to read locale from localStorage');
    }
    
    if (savedLocale && this.translations.has(savedLocale)) {
      this.currentLocale = savedLocale;
    } else {
      const browserLocale = this.detectBrowserLocale();
      this.currentLocale = browserLocale;
    }
    
    document.documentElement.lang = this.currentLocale.split('-')[0];
    console.log(`[I18n] Initialized with locale: ${this.currentLocale}`);
    
    return this.currentLocale;
  }

  /**
   * 检测浏览器语言
   * @returns {string} 检测到的语言代码
   */
  detectBrowserLocale() {
    const browserLang = navigator.language || navigator.userLanguage;
    
    if (this.translations.has(browserLang)) {
      return browserLang;
    }
    
    const baseLang = browserLang.split('-')[0];
    for (const locale of this.translations.keys()) {
      if (locale.startsWith(baseLang)) {
        return locale;
      }
    }
    
    return this.fallbackLocale;
  }

  /**
   * 翻译文本
   * @param {string} key - 翻译键，支持点分隔符嵌套 (如 'nav.dashboard')
   * @param {Object} params - 插值参数
   * @returns {string} 翻译后的文本
   */
  t(key, params = {}) {
    const messages = this.translations.get(this.currentLocale);
    let text = this.getNestedValue(messages, key);
    
    if (text === undefined) {
      const fallbackMessages = this.translations.get(this.fallbackLocale);
      text = this.getNestedValue(fallbackMessages, key);
    }
    
    if (text === undefined) {
      console.warn(`[I18n] Missing translation for key: "${key}"`);
      return key;
    }
    
    if (typeof text === 'string' && Object.keys(params).length > 0) {
      text = this.interpolate(text, params);
    }
    
    return text;
  }

  /**
   * 获取嵌套对象的值
   * @param {Object} obj - 源对象
   * @param {string} key - 点分隔的键路径
   * @returns {*} 找到的值或 undefined
   */
  getNestedValue(obj, key) {
    if (!obj || typeof key !== 'string') return undefined;
    
    const keys = key.split('.');
    let value = obj;
    
    for (const k of keys) {
      if (value && typeof value === 'object' && k in value) {
        value = value[k];
      } else {
        return undefined;
      }
    }
    
    return value;
  }

  /**
   * 插值替换参数
   * @param {string} text - 包含 {param} 占位符的文本
   * @param {Object} params - 参数对象
   * @returns {string} 替换后的文本
   */
  interpolate(text, params) {
    return text.replace(/\{(\w+)\}/g, (match, key) => {
      return params.hasOwnProperty(key) ? String(params[key]) : match;
    });
  }

  /**
   * 监听语言变化
   * @param {Function} callback - 回调函数 (newLocale, oldLocale) => void
   * @returns {Function} 取消监听函数
   */
  onLocaleChange(callback) {
    this.observers.add(callback);
    return () => this.observers.delete(callback);
  }

  /**
   * 更新页面中所有带有 data-i18n 属性的元素
   */
  updateDOM() {
    const elements = document.querySelectorAll('[data-i18n]');
    
    elements.forEach(element => {
      const key = element.getAttribute('data-i18n');
      const attr = element.getAttribute('data-i18n-attr') || 'textContent';
      const params = element.getAttribute('data-i18n-params');
      
      let parsedParams = {};
      if (params) {
        try {
          parsedParams = JSON.parse(params);
        } catch (e) {
          console.warn('[I18n] Failed to parse params:', params);
        }
      }
      
      const text = this.t(key, parsedParams);
      
      if (attr === 'textContent') {
        element.textContent = text;
      } else if (attr === 'innerHTML') {
        element.innerHTML = text;
      } else if (attr === 'placeholder') {
        element.placeholder = text;
      } else if (attr === 'title') {
        element.title = text;
      } else {
        element.setAttribute(attr, text);
      }
    });
  }
}

const i18n = new I18n();

export { i18n, I18n };
export default i18n;
