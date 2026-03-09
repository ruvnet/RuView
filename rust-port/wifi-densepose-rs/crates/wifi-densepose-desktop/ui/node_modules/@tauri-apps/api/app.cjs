'use strict';

var core = require('./core.cjs');
var image = require('./image.cjs');

// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
/**
 * Bundle type of the current application.
 */
exports.BundleType = void 0;
(function (BundleType) {
    /** Windows NSIS */
    BundleType["Nsis"] = "nsis";
    /** Windows MSI */
    BundleType["Msi"] = "msi";
    /** Linux Debian package */
    BundleType["Deb"] = "deb";
    /** Linux RPM */
    BundleType["Rpm"] = "rpm";
    /** Linux AppImage */
    BundleType["AppImage"] = "appimage";
    /** macOS app bundle */
    BundleType["App"] = "app";
})(exports.BundleType || (exports.BundleType = {}));
/**
 * Application metadata and related APIs.
 *
 * @module
 */
/**
 * Gets the application version.
 * @example
 * ```typescript
 * import { getVersion } from '@tauri-apps/api/app';
 * const appVersion = await getVersion();
 * ```
 *
 * @since 1.0.0
 */
async function getVersion() {
    return core.invoke('plugin:app|version');
}
/**
 * Gets the application name.
 * @example
 * ```typescript
 * import { getName } from '@tauri-apps/api/app';
 * const appName = await getName();
 * ```
 *
 * @since 1.0.0
 */
async function getName() {
    return core.invoke('plugin:app|name');
}
/**
 * Gets the Tauri framework version used by this application.
 *
 * @example
 * ```typescript
 * import { getTauriVersion } from '@tauri-apps/api/app';
 * const tauriVersion = await getTauriVersion();
 * ```
 *
 * @since 1.0.0
 */
async function getTauriVersion() {
    return core.invoke('plugin:app|tauri_version');
}
/**
 * Gets the application identifier.
 * @example
 * ```typescript
 * import { getIdentifier } from '@tauri-apps/api/app';
 * const identifier = await getIdentifier();
 * ```
 *
 * @returns The application identifier as configured in `tauri.conf.json`.
 *
 * @since 2.4.0
 */
async function getIdentifier() {
    return core.invoke('plugin:app|identifier');
}
/**
 * Shows the application on macOS. This function does not automatically
 * focus any specific app window.
 *
 * @example
 * ```typescript
 * import { show } from '@tauri-apps/api/app';
 * await show();
 * ```
 *
 * @since 1.2.0
 */
async function show() {
    return core.invoke('plugin:app|app_show');
}
/**
 * Hides the application on macOS.
 *
 * @example
 * ```typescript
 * import { hide } from '@tauri-apps/api/app';
 * await hide();
 * ```
 *
 * @since 1.2.0
 */
async function hide() {
    return core.invoke('plugin:app|app_hide');
}
/**
 * Fetches the data store identifiers on macOS and iOS.
 *
 * See https://developer.apple.com/documentation/webkit/wkwebsitedatastore for more information.
 *
 * @example
 * ```typescript
 * import { fetchDataStoreIdentifiers } from '@tauri-apps/api/app';
 * const ids = await fetchDataStoreIdentifiers();
 * ```
 *
 * @since 2.4.0
 */
async function fetchDataStoreIdentifiers() {
    return core.invoke('plugin:app|fetch_data_store_identifiers');
}
/**
 * Removes the data store with the given identifier.
 *
 * Note that any webview using this data store should be closed before running this API.
 *
 * See https://developer.apple.com/documentation/webkit/wkwebsitedatastore for more information.
 *
 * @example
 * ```typescript
 * import { fetchDataStoreIdentifiers, removeDataStore } from '@tauri-apps/api/app';
 * for (const id of (await fetchDataStoreIdentifiers())) {
 *   await removeDataStore(id);
 * }
 * ```
 *
 * @since 2.4.0
 */
async function removeDataStore(uuid) {
    return core.invoke('plugin:app|remove_data_store', { uuid });
}
/**
 * Gets the default window icon.
 *
 * @example
 * ```typescript
 * import { defaultWindowIcon } from '@tauri-apps/api/app';
 * const icon = await defaultWindowIcon();
 * ```
 *
 * @since 2.0.0
 */
async function defaultWindowIcon() {
    return core.invoke('plugin:app|default_window_icon').then((rid) => rid ? new image.Image(rid) : null);
}
/**
 * Sets the application's theme. Pass in `null` or `undefined` to follow
 * the system theme.
 *
 * @example
 * ```typescript
 * import { setTheme } from '@tauri-apps/api/app';
 * await setTheme('dark');
 * ```
 *
 * #### Platform-specific
 *
 * - **iOS / Android:** Unsupported.
 *
 * @since 2.0.0
 */
async function setTheme(theme) {
    return core.invoke('plugin:app|set_app_theme', { theme });
}
/**
 * Sets the dock visibility for the application on macOS.
 *
 * @param visible - Whether the dock should be visible or not.
 *
 * @example
 * ```typescript
 * import { setDockVisibility } from '@tauri-apps/api/app';
 * await setDockVisibility(false);
 * ```
 *
 * @since 2.5.0
 */
async function setDockVisibility(visible) {
    return core.invoke('plugin:app|set_dock_visibility', { visible });
}
/**
 * Gets the application bundle type.
 *
 * @example
 * ```typescript
 * import { getBundleType } from '@tauri-apps/api/app';
 * const type = await getBundleType();
 * ```
 *
 * @since 2.5.0
 */
async function getBundleType() {
    return core.invoke('plugin:app|bundle_type');
}
/**
 * Listens to the backButton event on Android.
 * @param handler
 */
async function onBackButtonPress(handler) {
    return core.addPluginListener('app', 'back-button', handler);
}

exports.defaultWindowIcon = defaultWindowIcon;
exports.fetchDataStoreIdentifiers = fetchDataStoreIdentifiers;
exports.getBundleType = getBundleType;
exports.getIdentifier = getIdentifier;
exports.getName = getName;
exports.getTauriVersion = getTauriVersion;
exports.getVersion = getVersion;
exports.hide = hide;
exports.onBackButtonPress = onBackButtonPress;
exports.removeDataStore = removeDataStore;
exports.setDockVisibility = setDockVisibility;
exports.setTheme = setTheme;
exports.show = show;
