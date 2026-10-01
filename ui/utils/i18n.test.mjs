import assert from 'node:assert/strict';
import test from 'node:test';

import { I18n } from './i18n.js';

test('Turkish locale is available and translates UI strings', () => {
  const originalDocument = globalThis.document;
  const originalLocalStorage = globalThis.localStorage;

  globalThis.document = {
    documentElement: { setAttribute() {} },
    querySelectorAll() { return []; },
    getElementById() { return null; }
  };
  globalThis.localStorage = {
    getItem() { return null; },
    setItem() {}
  };

  try {
    const i18n = new I18n();
    i18n.setLocale('tr');

    assert.ok(i18n.getAvailableLocales().includes('tr'));
    assert.equal(i18n.t('nav.dashboard'), 'Kontrol Paneli');
    assert.equal(i18n.t('conn.reconnecting'), 'Yeniden bağlanıyor...');
    assert.equal(i18n.t('unknown.key'), 'unknown.key');
  } finally {
    globalThis.document = originalDocument;
    globalThis.localStorage = originalLocalStorage;
  }
});

test('Turkish browser locales are detected automatically', () => {
  const originalNavigator = Object.getOwnPropertyDescriptor(globalThis, 'navigator');
  Object.defineProperty(globalThis, 'navigator', {
    configurable: true,
    value: { language: 'tr-TR' }
  });

  try {
    const i18n = new I18n();
    assert.equal(i18n.locale, 'tr');
  } finally {
    if (originalNavigator) {
      Object.defineProperty(globalThis, 'navigator', originalNavigator);
    } else {
      delete globalThis.navigator;
    }
  }
});
