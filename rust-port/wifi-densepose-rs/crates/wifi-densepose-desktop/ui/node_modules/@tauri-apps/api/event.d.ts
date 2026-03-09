declare global {
    interface Window {
        __TAURI_EVENT_PLUGIN_INTERNALS__: {
            unregisterListener: (event: string, eventId: number) => void;
        };
    }
}
type EventTarget = {
    kind: 'Any';
} | {
    kind: 'AnyLabel';
    label: string;
} | {
    kind: 'App';
} | {
    kind: 'Window';
    label: string;
} | {
    kind: 'Webview';
    label: string;
} | {
    kind: 'WebviewWindow';
    label: string;
};
interface Event<T> {
    /** Event name */
    event: EventName;
    /** Event identifier used to unlisten */
    id: number;
    /** Event payload */
    payload: T;
}
type EventCallback<T> = (event: Event<T>) => void;
type UnlistenFn = () => void;
type EventName = `${TauriEvent}` | (string & Record<never, never>);
interface Options {
    /**
     * The event target to listen to, defaults to `{ kind: 'Any' }`, see {@link EventTarget}.
     *
     * If a string is provided, {@link EventTarget.AnyLabel} is used.
     */
    target?: string | EventTarget;
}
/**
 * @since 1.1.0
 */
declare enum TauriEvent {
    WINDOW_RESIZED = "tauri://resize",
    WINDOW_MOVED = "tauri://move",
    WINDOW_CLOSE_REQUESTED = "tauri://close-requested",
    WINDOW_DESTROYED = "tauri://destroyed",
    WINDOW_FOCUS = "tauri://focus",
    WINDOW_BLUR = "tauri://blur",
    WINDOW_SCALE_FACTOR_CHANGED = "tauri://scale-change",
    WINDOW_THEME_CHANGED = "tauri://theme-changed",
    WINDOW_CREATED = "tauri://window-created",
    WEBVIEW_CREATED = "tauri://webview-created",
    DRAG_ENTER = "tauri://drag-enter",
    DRAG_OVER = "tauri://drag-over",
    DRAG_DROP = "tauri://drag-drop",
    DRAG_LEAVE = "tauri://drag-leave"
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
declare function listen<T>(event: EventName, handler: EventCallback<T>, options?: Options): Promise<UnlistenFn>;
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
declare function once<T>(event: EventName, handler: EventCallback<T>, options?: Options): Promise<UnlistenFn>;
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
declare function emit<T>(event: string, payload?: T): Promise<void>;
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
declare function emitTo<T>(target: EventTarget | string, event: string, payload?: T): Promise<void>;
export type { Event, EventTarget, EventCallback, UnlistenFn, EventName, Options };
export { listen, once, emit, emitTo, TauriEvent };
