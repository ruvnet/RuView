import { PhysicalPosition, PhysicalSize, Size, Position } from './dpi.js';
import { listen, once, emit, emitTo, TauriEvent } from './event.js';
import { invoke } from './core.js';
import { Window, getCurrentWindow } from './window.js';

// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
/**
 * Provides APIs to create webviews, communicate with other webviews and manipulate the current webview.
 *
 * #### Webview events
 *
 * Events can be listened to using {@link Webview.listen}:
 * ```typescript
 * import { getCurrentWebview } from "@tauri-apps/api/webview";
 * getCurrentWebview().listen("my-webview-event", ({ event, payload }) => { });
 * ```
 *
 * @module
 */
/**
 * Get an instance of `Webview` for the current webview.
 *
 * @since 2.0.0
 */
function getCurrentWebview() {
    return new Webview(getCurrentWindow(), window.__TAURI_INTERNALS__.metadata.currentWebview.label, {
        // @ts-expect-error `skip` is not defined in the public API but it is handled by the constructor
        skip: true
    });
}
/**
 * Gets a list of instances of `Webview` for all available webviews.
 *
 * @since 2.0.0
 */
async function getAllWebviews() {
    return invoke('plugin:webview|get_all_webviews').then((webviews) => webviews.map((w) => new Webview(new Window(w.windowLabel, {
        // @ts-expect-error `skip` is not defined in the public API but it is handled by the constructor
        skip: true
    }), w.label, {
        // @ts-expect-error `skip` is not defined in the public API but it is handled by the constructor
        skip: true
    })));
}
/** @ignore */
// events that are emitted right here instead of by the created webview
const localTauriEvents = ['tauri://created', 'tauri://error'];
/**
 * Create new webview or get a handle to an existing one.
 *
 * Webviews are identified by a *label*  a unique identifier that can be used to reference it later.
 * It may only contain alphanumeric characters `a-zA-Z` plus the following special characters `-`, `/`, `:` and `_`.
 *
 * @example
 * ```typescript
 * import { Window } from "@tauri-apps/api/window"
 * import { Webview } from "@tauri-apps/api/webview"
 *
 * const appWindow = new Window('uniqueLabel');
 *
 * appWindow.once('tauri://created', async function () {
 *   // `new Webview` Should be called after the window is successfully created,
 *   // or webview may not be attached to the window since window is not created yet.
 *
 *   // loading embedded asset:
 *   const webview = new Webview(appWindow, 'theUniqueLabel', {
 *     url: 'path/to/page.html',
 *
 *     // create a webview with specific logical position and size
 *     x: 0,
 *     y: 0,
 *     width: 800,
 *     height: 600,
 *   });
 *   // alternatively, load a remote URL:
 *   const webview = new Webview(appWindow, 'theUniqueLabel', {
 *     url: 'https://github.com/tauri-apps/tauri',
 *
 *     // create a webview with specific logical position and size
 *     x: 0,
 *     y: 0,
 *     width: 800,
 *     height: 600,
 *   });
 *
 *   webview.once('tauri://created', function () {
 *     // webview successfully created
 *   });
 *   webview.once('tauri://error', function (e) {
 *     // an error happened creating the webview
 *   });
 *
 *
 *   // emit an event to the backend
 *   await webview.emit("some-event", "data");
 *   // listen to an event from the backend
 *   const unlisten = await webview.listen("event-name", e => { });
 *   unlisten();
 * });
 * ```
 *
 * @since 2.0.0
 */
class Webview {
    /**
     * Creates a new Webview.
     * @example
     * ```typescript
     * import { Window } from '@tauri-apps/api/window'
     * import { Webview } from '@tauri-apps/api/webview'
     * const appWindow = new Window('my-label')
     *
     * appWindow.once('tauri://created', async function() {
     *   const webview = new Webview(appWindow, 'my-label', {
     *     url: 'https://github.com/tauri-apps/tauri',
     *
     *     // create a webview with specific logical position and size
     *     x: 0,
     *     y: 0,
     *     width: 800,
     *     height: 600,
     *   });
     *
     *   webview.once('tauri://created', function () {
     *     // webview successfully created
     *   });
     *   webview.once('tauri://error', function (e) {
     *     // an error happened creating the webview
     *   });
     * });
     * ```
     *
     * @param window the window to add this webview to.
     * @param label The unique webview label. Must be alphanumeric: `a-zA-Z-/:_`.
     * @returns The {@link Webview} instance to communicate with the webview.
     */
    constructor(window, label, options) {
        this.window = window;
        this.label = label;
        // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment
        this.listeners = Object.create(null);
        // @ts-expect-error `skip` is not a public API so it is not defined in WebviewOptions
        if (!(options === null || options === void 0 ? void 0 : options.skip)) {
            invoke('plugin:webview|create_webview', {
                windowLabel: window.label,
                options: {
                    ...options,
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
     * import { Webview } from '@tauri-apps/api/webview';
     * const mainWebview = Webview.getByLabel('main');
     * ```
     *
     * @param label The webview label.
     * @returns The Webview instance to communicate with the webview or null if the webview doesn't exist.
     */
    static async getByLabel(label) {
        var _a;
        return (_a = (await getAllWebviews()).find((w) => w.label === label)) !== null && _a !== void 0 ? _a : null;
    }
    /**
     * Get an instance of `Webview` for the current webview.
     */
    static getCurrent() {
        return getCurrentWebview();
    }
    /**
     * Gets a list of instances of `Webview` for all available webviews.
     */
    static async getAll() {
        return getAllWebviews();
    }
    /**
     * Listen to an emitted event on this webview.
     *
     * @example
     * ```typescript
     * import { getCurrentWebview } from '@tauri-apps/api/webview';
     * const unlisten = await getCurrentWebview().listen<string>('state-changed', (event) => {
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
    async listen(event, handler) {
        if (this._handleTauriEvent(event, handler)) {
            return () => {
                // eslint-disable-next-line security/detect-object-injection
                const listeners = this.listeners[event];
                listeners.splice(listeners.indexOf(handler), 1);
            };
        }
        return listen(event, handler, {
            target: { kind: 'Webview', label: this.label }
        });
    }
    /**
     * Listen to an emitted event on this webview only once.
     *
     * @example
     * ```typescript
     * import { getCurrentWebview } from '@tauri-apps/api/webview';
     * const unlisten = await getCurrent().once<null>('initialized', (event) => {
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
    async once(event, handler) {
        if (this._handleTauriEvent(event, handler)) {
            return () => {
                // eslint-disable-next-line security/detect-object-injection
                const listeners = this.listeners[event];
                listeners.splice(listeners.indexOf(handler), 1);
            };
        }
        return once(event, handler, {
            target: { kind: 'Webview', label: this.label }
        });
    }
    /**
     * Emits an event to all {@link EventTarget|targets}.
     *
     * @example
     * ```typescript
     * import { getCurrentWebview } from '@tauri-apps/api/webview';
     * await getCurrentWebview().emit('webview-loaded', { loggedIn: true, token: 'authToken' });
     * ```
     *
     * @param event Event name. Must include only alphanumeric characters, `-`, `/`, `:` and `_`.
     * @param payload Event payload.
     */
    async emit(event, payload) {
        if (localTauriEvents.includes(event)) {
            // eslint-disable-next-line
            for (const handler of this.listeners[event] || []) {
                handler({
                    event,
                    id: -1,
                    payload
                });
            }
            return;
        }
        return emit(event, payload);
    }
    /**
     * Emits an event to all {@link EventTarget|targets} matching the given target.
     *
     * @example
     * ```typescript
     * import { getCurrentWebview } from '@tauri-apps/api/webview';
     * await getCurrentWebview().emitTo('main', 'webview-loaded', { loggedIn: true, token: 'authToken' });
     * ```
     *
     * @param target Label of the target Window/Webview/WebviewWindow or raw {@link EventTarget} object.
     * @param event Event name. Must include only alphanumeric characters, `-`, `/`, `:` and `_`.
     * @param payload Event payload.
     */
    async emitTo(target, event, payload) {
        if (localTauriEvents.includes(event)) {
            // eslint-disable-next-line
            for (const handler of this.listeners[event] || []) {
                handler({
                    event,
                    id: -1,
                    payload
                });
            }
            return;
        }
        return emitTo(target, event, payload);
    }
    /** @ignore */
    _handleTauriEvent(event, handler) {
        if (localTauriEvents.includes(event)) {
            if (!(event in this.listeners)) {
                // eslint-disable-next-line security/detect-object-injection
                this.listeners[event] = [handler];
            }
            else {
                // eslint-disable-next-line security/detect-object-injection
                this.listeners[event].push(handler);
            }
            return true;
        }
        return false;
    }
    // Getters
    /**
     * The position of the top-left hand corner of the webview's client area relative to the top-left hand corner of the desktop.
     * @example
     * ```typescript
     * import { getCurrentWebview } from '@tauri-apps/api/webview';
     * const position = await getCurrentWebview().position();
     * ```
     *
     * @returns The webview's position.
     */
    async position() {
        return invoke('plugin:webview|webview_position', {
            label: this.label
        }).then((p) => new PhysicalPosition(p));
    }
    /**
     * The physical size of the webview's client area.
     * The client area is the content of the webview, excluding the title bar and borders.
     * @example
     * ```typescript
     * import { getCurrentWebview } from '@tauri-apps/api/webview';
     * const size = await getCurrentWebview().size();
     * ```
     *
     * @returns The webview's size.
     */
    async size() {
        return invoke('plugin:webview|webview_size', {
            label: this.label
        }).then((s) => new PhysicalSize(s));
    }
    // Setters
    /**
     * Closes the webview.
     * @example
     * ```typescript
     * import { getCurrentWebview } from '@tauri-apps/api/webview';
     * await getCurrentWebview().close();
     * ```
     *
     * @returns A promise indicating the success or failure of the operation.
     */
    async close() {
        return invoke('plugin:webview|webview_close', {
            label: this.label
        });
    }
    /**
     * Resizes the webview.
     * @example
     * ```typescript
     * import { getCurrent, LogicalSize } from '@tauri-apps/api/webview';
     * await getCurrentWebview().setSize(new LogicalSize(600, 500));
     * ```
     *
     * @param size The logical or physical size.
     * @returns A promise indicating the success or failure of the operation.
     */
    async setSize(size) {
        return invoke('plugin:webview|set_webview_size', {
            label: this.label,
            value: size instanceof Size ? size : new Size(size)
        });
    }
    /**
     * Sets the webview position.
     * @example
     * ```typescript
     * import { getCurrent, LogicalPosition } from '@tauri-apps/api/webview';
     * await getCurrentWebview().setPosition(new LogicalPosition(600, 500));
     * ```
     *
     * @param position The new position, in logical or physical pixels.
     * @returns A promise indicating the success or failure of the operation.
     */
    async setPosition(position) {
        return invoke('plugin:webview|set_webview_position', {
            label: this.label,
            value: position instanceof Position ? position : new Position(position)
        });
    }
    /**
     * Bring the webview to front and focus.
     * @example
     * ```typescript
     * import { getCurrentWebview } from '@tauri-apps/api/webview';
     * await getCurrentWebview().setFocus();
     * ```
     *
     * @returns A promise indicating the success or failure of the operation.
     */
    async setFocus() {
        return invoke('plugin:webview|set_webview_focus', {
            label: this.label
        });
    }
    /**
     * Sets whether the webview should automatically grow and shrink its size and position when the parent window resizes.
     * @example
     * ```typescript
     * import { getCurrentWebview } from '@tauri-apps/api/webview';
     * await getCurrentWebview().setAutoResize(true);
     * ```
     *
     * @returns A promise indicating the success or failure of the operation.
     */
    async setAutoResize(autoResize) {
        return invoke('plugin:webview|set_webview_auto_resize', {
            label: this.label,
            value: autoResize
        });
    }
    /**
     * Hide the webview.
     * @example
     * ```typescript
     * import { getCurrentWebview } from '@tauri-apps/api/webview';
     * await getCurrentWebview().hide();
     * ```
     *
     * @returns A promise indicating the success or failure of the operation.
     */
    async hide() {
        return invoke('plugin:webview|webview_hide', {
            label: this.label
        });
    }
    /**
     * Show the webview.
     * @example
     * ```typescript
     * import { getCurrentWebview } from '@tauri-apps/api/webview';
     * await getCurrentWebview().show();
     * ```
     *
     * @returns A promise indicating the success or failure of the operation.
     */
    async show() {
        return invoke('plugin:webview|webview_show', {
            label: this.label
        });
    }
    /**
     * Set webview zoom level.
     * @example
     * ```typescript
     * import { getCurrentWebview } from '@tauri-apps/api/webview';
     * await getCurrentWebview().setZoom(1.5);
     * ```
     *
     * @returns A promise indicating the success or failure of the operation.
     */
    async setZoom(scaleFactor) {
        return invoke('plugin:webview|set_webview_zoom', {
            label: this.label,
            value: scaleFactor
        });
    }
    /**
     * Moves this webview to the given label.
     * @example
     * ```typescript
     * import { getCurrentWebview } from '@tauri-apps/api/webview';
     * await getCurrentWebview().reparent('other-window');
     * ```
     *
     * @returns A promise indicating the success or failure of the operation.
     */
    async reparent(window) {
        return invoke('plugin:webview|reparent', {
            label: this.label,
            window: typeof window === 'string' ? window : window.label
        });
    }
    /**
     * Clears all browsing data for this webview.
     * @example
     * ```typescript
     * import { getCurrentWebview } from '@tauri-apps/api/webview';
     * await getCurrentWebview().clearAllBrowsingData();
     * ```
     *
     * @returns A promise indicating the success or failure of the operation.
     */
    async clearAllBrowsingData() {
        return invoke('plugin:webview|clear_all_browsing_data');
    }
    /**
     * Specify the webview background color.
     *
     * #### Platfrom-specific:
     *
     * - **macOS / iOS**: Not implemented.
     * - **Windows**:
     *   - On Windows 7, transparency is not supported and the alpha value will be ignored.
     *   - On Windows higher than 7: translucent colors are not supported so any alpha value other than `0` will be replaced by `255`
     *
     * @returns A promise indicating the success or failure of the operation.
     *
     * @since 2.1.0
     */
    async setBackgroundColor(color) {
        return invoke('plugin:webview|set_webview_background_color', { color });
    }
    // Listeners
    /**
     * Listen to a file drop event.
     * The listener is triggered when the user hovers the selected files on the webview,
     * drops the files or cancels the operation.
     *
     * @example
     * ```typescript
     * import { getCurrentWebview } from "@tauri-apps/api/webview";
     * const unlisten = await getCurrentWebview().onDragDropEvent((event) => {
     *  if (event.payload.type === 'over') {
     *    console.log('User hovering', event.payload.position);
     *  } else if (event.payload.type === 'drop') {
     *    console.log('User dropped', event.payload.paths);
     *  } else {
     *    console.log('File drop cancelled');
     *  }
     * });
     *
     * // you need to call unlisten if your handler goes out of scope e.g. the component is unmounted
     * unlisten();
     * ```
     *
     * When the debugger panel is open, the drop position of this event may be inaccurate due to a known limitation.
     * To retrieve the correct drop position, please detach the debugger.
     *
     * @returns A promise resolving to a function to unlisten to the event.
     * Note that removing the listener is required if your listener goes out of scope e.g. the component is unmounted.
     */
    async onDragDropEvent(handler) {
        const unlistenDragEnter = await this.listen(TauriEvent.DRAG_ENTER, (event) => {
            handler({
                ...event,
                payload: {
                    type: 'enter',
                    paths: event.payload.paths,
                    position: new PhysicalPosition(event.payload.position)
                }
            });
        });
        const unlistenDragOver = await this.listen(TauriEvent.DRAG_OVER, (event) => {
            handler({
                ...event,
                payload: {
                    type: 'over',
                    position: new PhysicalPosition(event.payload.position)
                }
            });
        });
        const unlistenDragDrop = await this.listen(TauriEvent.DRAG_DROP, (event) => {
            handler({
                ...event,
                payload: {
                    type: 'drop',
                    paths: event.payload.paths,
                    position: new PhysicalPosition(event.payload.position)
                }
            });
        });
        const unlistenDragLeave = await this.listen(TauriEvent.DRAG_LEAVE, (event) => {
            handler({ ...event, payload: { type: 'leave' } });
        });
        return () => {
            unlistenDragEnter();
            unlistenDragDrop();
            unlistenDragOver();
            unlistenDragLeave();
        };
    }
}

export { Webview, getAllWebviews, getCurrentWebview };
