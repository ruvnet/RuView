'use strict';

var webview = require('./webview.cjs');
var window = require('./window.cjs');
var event = require('./event.cjs');
var core = require('./core.cjs');

// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
/**
 * Get an instance of `Webview` for the current webview window.
 *
 * @since 2.0.0
 */
function getCurrentWebviewWindow() {
    const webview$1 = webview.getCurrentWebview();
    // @ts-expect-error `skip` is not defined in the public API but it is handled by the constructor
    return new WebviewWindow(webview$1.label, { skip: true });
}
/**
 * Gets a list of instances of `Webview` for all available webview windows.
 *
 * @since 2.0.0
 */
async function getAllWebviewWindows() {
    return core.invoke('plugin:window|get_all_windows').then((windows) => windows.map((w) => new WebviewWindow(w, {
        // @ts-expect-error `skip` is not defined in the public API but it is handled by the constructor
        skip: true
    })));
}
// eslint-disable-next-line @typescript-eslint/no-unsafe-declaration-merging
class WebviewWindow {
    /**
     * Creates a new {@link Window} hosting a {@link Webview}.
     * @example
     * ```typescript
     * import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
     * const webview = new WebviewWindow('my-label', {
     *   url: 'https://github.com/tauri-apps/tauri'
     * });
     * webview.once('tauri://created', function () {
     *  // webview successfully created
     * });
     * webview.once('tauri://error', function (e) {
     *  // an error happened creating the webview
     * });
     * ```
     *
     * @param label The unique webview label. Must be alphanumeric: `a-zA-Z-/:_`.
     * @returns The {@link WebviewWindow} instance to communicate with the window and webview.
     */
    constructor(label, options = {}) {
        var _a;
        this.label = label;
        // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment
        this.listeners = Object.create(null);
        // @ts-expect-error `skip` is not a public API so it is not defined in WebviewOptions
        if (!(options === null || options === void 0 ? void 0 : options.skip)) {
            core.invoke('plugin:webview|create_webview_window', {
                options: {
                    ...options,
                    parent: typeof options.parent === 'string'
                        ? options.parent
                        : (_a = options.parent) === null || _a === void 0 ? void 0 : _a.label,
                    label
                }
            })
                .then(async () => this.emit('tauri://created'))
                .catch(async (e) => this.emit('tauri://error', e));
        }
    }
    /**
     * Gets the Webview for the webview associated with the given label.
     * @example
     * ```typescript
     * import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
     * const mainWebview = WebviewWindow.getByLabel('main');
     * ```
     *
     * @param label The webview label.
     * @returns The Webview instance to communicate with the webview or null if the webview doesn't exist.
     */
    static async getByLabel(label) {
        var _a;
        const webview = (_a = (await getAllWebviewWindows()).find((w) => w.label === label)) !== null && _a !== void 0 ? _a : null;
        if (webview) {
            // @ts-expect-error `skip` is not defined in the public API but it is handled by the constructor
            return new WebviewWindow(webview.label, { skip: true });
        }
        return null;
    }
    /**
     * Get an instance of `Webview` for the current webview.
     */
    static getCurrent() {
        return getCurrentWebviewWindow();
    }
    /**
     * Gets a list of instances of `Webview` for all available webviews.
     */
    static async getAll() {
        return getAllWebviewWindows();
    }
    /**
     * Listen to an emitted event on this webview window.
     *
     * @example
     * ```typescript
     * import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
     * const unlisten = await WebviewWindow.getCurrent().listen<string>('state-changed', (event) => {
     *   console.log(`Got error: ${payload}`);
     * });
     *
     * // you need to call unlisten if your handler goes out of scope e.g. the component is unmounted
     * unlisten();
     * ```
     *
     * @param event Event name. Must include only alphanumeric characters, `-`, `/`, `:` and `_`.
     * @param handler Event handler.
     * @returns A promise resolving to a function to unlisten to the event.
     * Note that removing the listener is required if your listener goes out of scope e.g. the component is unmounted.
     */
    async listen(event$1, handler) {
        if (this._handleTauriEvent(event$1, handler)) {
            return () => {
                // eslint-disable-next-line security/detect-object-injection
                const listeners = this.listeners[event$1];
                listeners.splice(listeners.indexOf(handler), 1);
            };
        }
        return event.listen(event$1, handler, {
            target: { kind: 'WebviewWindow', label: this.label }
        });
    }
    /**
     * Listen to an emitted event on this webview window only once.
     *
     * @example
     * ```typescript
     * import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
     * const unlisten = await WebviewWindow.getCurrent().once<null>('initialized', (event) => {
     *   console.log(`Webview initialized!`);
     * });
     *
     * // you need to call unlisten if your handler goes out of scope e.g. the component is unmounted
     * unlisten();
     * ```
     *
     * @param event Event name. Must include only alphanumeric characters, `-`, `/`, `:` and `_`.
     * @param handler Event handler.
     * @returns A promise resolving to a function to unlisten to the event.
     * Note that removing the listener is required if your listener goes out of scope e.g. the component is unmounted.
     */
    async once(event$1, handler) {
        if (this._handleTauriEvent(event$1, handler)) {
            return () => {
                // eslint-disable-next-line security/detect-object-injection
                const listeners = this.listeners[event$1];
                listeners.splice(listeners.indexOf(handler), 1);
            };
        }
        return event.once(event$1, handler, {
            target: { kind: 'WebviewWindow', label: this.label }
        });
    }
    /**
     * Set the window and webview background color.
     *
     * #### Platform-specific:
     *
     * - **Android / iOS:** Unsupported for the window layer.
     * - **macOS / iOS**: Not implemented for the webview layer.
     * - **Windows**:
     *   - alpha channel is ignored for the window layer.
     *   - On Windows 7, alpha channel is ignored for the webview layer.
     *   - On Windows 8 and newer, if alpha channel is not `0`, it will be ignored.
     *
     * @returns A promise indicating the success or failure of the operation.
     *
     * @since 2.1.0
     */
    async setBackgroundColor(color) {
        return core.invoke('plugin:window|set_background_color', { color }).then(() => {
            return core.invoke('plugin:webview|set_webview_background_color', { color });
        });
    }
}
// Order matters, we use window APIs by default
applyMixins(WebviewWindow, [window.Window, webview.Webview]);
/** Extends a base class by other specified classes, without overriding existing properties */
function applyMixins(baseClass, extendedClasses) {
    (Array.isArray(extendedClasses)
        ? extendedClasses
        : [extendedClasses]).forEach((extendedClass) => {
        Object.getOwnPropertyNames(extendedClass.prototype).forEach((name) => {
            var _a;
            if (typeof baseClass.prototype === 'object'
                && baseClass.prototype
                && name in baseClass.prototype)
                return;
            Object.defineProperty(baseClass.prototype, name, 
            // eslint-disable-next-line
            (_a = Object.getOwnPropertyDescriptor(extendedClass.prototype, name)) !== null && _a !== void 0 ? _a : Object.create(null));
        });
    });
}

exports.WebviewWindow = WebviewWindow;
exports.getAllWebviewWindows = getAllWebviewWindows;
exports.getCurrentWebviewWindow = getCurrentWebviewWindow;
