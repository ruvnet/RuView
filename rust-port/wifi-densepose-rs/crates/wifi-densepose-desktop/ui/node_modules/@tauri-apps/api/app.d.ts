import { PluginListener } from './core';
import { Image } from './image';
import { Theme } from './window';
/**
 * Identifier type used for data stores on macOS and iOS.
 *
 * Represents a 128-bit identifier, commonly expressed as a 16-byte UUID.
 */
export type DataStoreIdentifier = [
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number
];
/**
 * Bundle type of the current application.
 */
export declare enum BundleType {
    /** Windows NSIS */
    Nsis = "nsis",
    /** Windows MSI */
    Msi = "msi",
    /** Linux Debian package */
    Deb = "deb",
    /** Linux RPM */
    Rpm = "rpm",
    /** Linux AppImage */
    AppImage = "appimage",
    /** macOS app bundle */
    App = "app"
}
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
declare function getVersion(): Promise<string>;
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
declare function getName(): Promise<string>;
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
declare function getTauriVersion(): Promise<string>;
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
declare function getIdentifier(): Promise<string>;
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
declare function show(): Promise<void>;
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
declare function hide(): Promise<void>;
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
declare function fetchDataStoreIdentifiers(): Promise<DataStoreIdentifier[]>;
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
declare function removeDataStore(uuid: DataStoreIdentifier): Promise<void>;
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
declare function defaultWindowIcon(): Promise<Image | null>;
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
declare function setTheme(theme?: Theme | null): Promise<void>;
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
declare function setDockVisibility(visible: boolean): Promise<void>;
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
declare function getBundleType(): Promise<BundleType>;
/**
 * Payload for the onBackButtonPress event.
 */
type OnBackButtonPressPayload = {
    /** Whether the webview canGoBack property is true. */
    canGoBack: boolean;
};
/**
 * Listens to the backButton event on Android.
 * @param handler
 */
declare function onBackButtonPress(handler: (payload: OnBackButtonPressPayload) => void): Promise<PluginListener>;
export { getName, getVersion, getTauriVersion, getIdentifier, show, hide, defaultWindowIcon, setTheme, fetchDataStoreIdentifiers, removeDataStore, setDockVisibility, getBundleType, type OnBackButtonPressPayload, onBackButtonPress };
