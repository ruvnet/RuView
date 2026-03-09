'use strict';

var core = require('./core.cjs');

// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
/**
 * The event system allows you to emit events to the backend and listen to events from it.
 *
 * This package is also accessible with `window.__TAURI__.event` when [`app.withGlobalTauri`](https://v2.tauri.app/reference/config/#withglobaltauri) in `tauri.conf.json` is set to `true`.
 * @module
 */
/**
 * @since 1.1.0
 */
exports.TauriEvent = void 0;
(function (TauriEvent) {
    TauriEvent["WINDOW_RESIZED"] = "tauri://resize";
    TauriEvent["WINDOW_MOVED"] = "tauri://move";
    TauriEvent["WINDOW_CLOSE_REQUESTED"] = "tauri://close-requested";
    TauriEvent["WINDOW_DESTROYED"] = "tauri://destroyed";
    TauriEvent["WINDOW_FOCUS"] = "tauri://focus";
    TauriEvent["WINDOW_BLUR"] = "tauri://blur";
    TauriEvent["WINDOW_SCALE_FACTOR_CHANGED"] = "tauri://scale-change";
    TauriEvent["WINDOW_THEME_CHANGED"] = "tauri://theme-changed";
    TauriEvent["WINDOW_CREATED"] = "tauri://window-created";
    TauriEvent["WEBVIEW_CREATED"] = "tauri://webview-created";
    TauriEvent["DRAG_ENTER"] = "tauri://drag-enter";
    TauriEvent["DRAG_OVER"] = "tauri://drag-over";
    TauriEvent["DRAG_DROP"] = "tauri://drag-drop";
    TauriEvent["DRAG_LEAVE"] = "tauri://drag-leave";
})(exports.TauriEvent || (exports.TauriEvent = {}));
/**
 * Unregister the event listener associated with the given name and id.
 *
 * @ignore
 * @param event The event name
 * @param eventId Event identifier
 * @returns
 */
async function _unlisten(event, eventId) {
    window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener(event, eventId);
    await core.invoke('plugin:event|unlisten', {
        event,
        eventId
    });
}
/**
 * Listen to an emitted event to any {@link EventTarget|target}.
 *
 * @example
 * ```typescript
 * import { listen } from '@tauri-apps/api/event';
 * const unlisten = await listen<string>('error', (event) => {
 *   console.log(`Got error, payload: ${event.payload}`);
 * });
 *
 * // you need to call unlisten if your handler goes out of scope e.g. the component is unmounted
 * unlisten();
 * ```
 *
 * @param event Event name. Must include only alphanumeric characters, `-`, `/`, `:` and `_`.
 * @param handler Event handler callback.
 * @param options Event listening options.
 * @returns A promise resolving to a function to unlisten to the event.
 * Note that removing the listener is required if your listener goes out of scope e.g. the component is unmounted.
 *
 * @since 1.0.0
 */
async function listen(event, handler, options) {
    var _a;
    const target = typeof (options === null || options === void 0 ? void 0 : options.target) === 'string'
        ? { kind: 'AnyLabel', label: options.target }
        : ((_a = options === null || options === void 0 ? void 0 : options.target) !== null && _a !== void 0 ? _a : { kind: 'Any' });
    return core.invoke('plugin:event|listen', {
        event,
        target,
        handler: core.transformCallback(handler)
    }).then((eventId) => {
        return async () => _unlisten(event, eventId);
    });
}
/**
 * Listens once to an emitted event to any {@link EventTarget|target}.
 *
 * @example
 * ```typescript
 * import { once } from '@tauri-apps/api/event';
 * interface LoadedPayload {
 *   loggedIn: boolean,
 *   token: string
 * }
 * const unlisten = await once<LoadedPayload>('loaded', (event) => {
 *   console.log(`App is loaded, loggedIn: ${event.payload.loggedIn}, token: ${event.payload.token}`);
 * });
 *
 * // you need to call unlisten if your handler goes out of scope e.g. the component is unmounted
 * unlisten();
 * ```
 *
 * @param event Event name. Must include only alphanumeric characters, `-`, `/`, `:` and `_`.
 * @param handler Event handler callback.
 * @param options Event listening options.
 * @returns A promise resolving to a function to unlisten to the event.
 * Note that removing the listener is required if your listener goes out of scope e.g. the component is unmounted.
 *
 * @since 1.0.0
 */
async function once(event, handler, options) {
    return listen(event, (eventData) => {
        void _unlisten(event, eventData.id);
        handler(eventData);
    }, options);
}
/**
 * Emits an event to all {@link EventTarget|targets}.
 *
 * @example
 * ```typescript
 * import { emit } from '@tauri-apps/api/event';
 * await emit('frontend-loaded', { loggedIn: true, token: 'authToken' });
 * ```
 *
 * @param event Event name. Must include only alphanumeric characters, `-`, `/`, `:` and `_`.
 * @param payload Event payload.
 *
 * @since 1.0.0
 */
async function emit(event, payload) {
    await core.invoke('plugin:event|emit', {
        event,
        payload
    });
}
/**
 * Emits an event to all {@link EventTarget|targets} matching the given target.
 *
 * @example
 * ```typescript
 * import { emitTo } from '@tauri-apps/api/event';
 * await emitTo('main', 'frontend-loaded', { loggedIn: true, token: 'authToken' });
 * ```
 *
 * @param target Label of the target Window/Webview/WebviewWindow or raw {@link EventTarget} object.
 * @param event Event name. Must include only alphanumeric characters, `-`, `/`, `:` and `_`.
 * @param payload Event payload.
 *
 * @since 2.0.0
 */
async function emitTo(target, event, payload) {
    const eventTarget = typeof target === 'string' ? { kind: 'AnyLabel', label: target } : target;
    await core.invoke('plugin:event|emit_to', {
        target: eventTarget,
        event,
        payload
    });
}

exports.emit = emit;
exports.emitTo = emitTo;
exports.listen = listen;
exports.once = once;
