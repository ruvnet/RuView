import { Webview, WebviewLabel, WebviewOptions } from './webview';
import type { WindowOptions } from './window';
import { Window } from './window';
import type { EventName, EventCallback, UnlistenFn } from './event';
import type { Color, DragDropEvent } from './webview';
/**
 * Get an instance of `Webview` for the current webview window.
 *
 * @since 2.0.0
 */
declare function getCurrentWebviewWindow(): WebviewWindow;
/**
 * Gets a list of instances of `Webview` for all available webview windows.
 *
 * @since 2.0.0
 */
declare function getAllWebviewWindows(): Promise<WebviewWindow[]>;
interface WebviewWindow extends Webview, Window {
}
declare class WebviewWindow {
    label: string;
    /** Local event listeners. */
    listeners: Record<string, Array<EventCallback<any>>>;
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
    constructor(label: WebviewLabel, options?: Omit<WebviewOptions, 'x' | 'y' | 'width' | 'height'> & WindowOptions);
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
    static getByLabel(label: string): Promise<WebviewWindow | null>;
    /**
     * Get an instance of `Webview` for the current webview.
     */
    static getCurrent(): WebviewWindow;
    /**
     * Gets a list of instances of `Webview` for all available webviews.
     */
    static getAll(): Promise<WebviewWindow[]>;
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
    listen<T>(event: EventName, handler: EventCallback<T>): Promise<UnlistenFn>;
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
    once<T>(event: EventName, handler: EventCallback<T>): Promise<UnlistenFn>;
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
    setBackgroundColor(color: Color): Promise<void>;
}
export { WebviewWindow, getCurrentWebviewWindow, getAllWebviewWindows };
export type { DragDropEvent, Color };
