/**
 * Invoke your custom commands.
 *
 * This package is also accessible with `window.__TAURI__.core` when [`app.withGlobalTauri`](https://v2.tauri.app/reference/config/#withglobaltauri) in `tauri.conf.json` is set to `true`.
 * @module
 */
/**
 * A key to be used to implement a special function
 * on your types that define how your type should be serialized
 * when passing across the IPC.
 * @example
 * Given a type in Rust that looks like this
 * ```rs
 * #[derive(serde::Serialize, serde::Deserialize)
 * enum UserId {
 *   String(String),
 *   Number(u32),
 * }
 * ```
 * `UserId::String("id")` would be serialized into `{ String: "id" }`
 * and so we need to pass the same structure back to Rust
 * ```ts
 * import { SERIALIZE_TO_IPC_FN } from "@tauri-apps/api/core"
 *
 * class UserIdString {
 *   id
 *   constructor(id) {
 *     this.id = id
 *   }
 *
 *   [SERIALIZE_TO_IPC_FN]() {
 *     return { String: this.id }
 *   }
 * }
 *
 * class UserIdNumber {
 *   id
 *   constructor(id) {
 *     this.id = id
 *   }
 *
 *   [SERIALIZE_TO_IPC_FN]() {
 *     return { Number: this.id }
 *   }
 * }
 *
 * type UserId = UserIdString | UserIdNumber
 * ```
 *
 */
export declare const SERIALIZE_TO_IPC_FN = "__TAURI_TO_IPC_KEY__";
/**
 * Stores the callback in a known location, and returns an identifier that can be passed to the backend.
 * The backend uses the identifier to `eval()` the callback.
 *
 * @return An unique identifier associated with the callback function.
 *
 * @since 1.0.0
 */
declare function transformCallback<T = unknown>(callback?: (response: T) => void, once?: boolean): number;
declare class Channel<T = unknown> {
    #private;
    /** The callback id returned from {@linkcode transformCallback} */
    id: number;
    constructor(onmessage?: (response: T) => void);
    private cleanupCallback;
    set onmessage(handler: (response: T) => void);
    get onmessage(): (response: T) => void;
    [SERIALIZE_TO_IPC_FN](): string;
    toJSON(): string;
}
declare class PluginListener {
    plugin: string;
    event: string;
    channelId: number;
    constructor(plugin: string, event: string, channelId: number);
    unregister(): Promise<void>;
}
/**
 * Adds a listener to a plugin event.
 *
 * @returns The listener object to stop listening to the events.
 *
 * @since 2.0.0
 */
declare function addPluginListener<T>(plugin: string, event: string, cb: (payload: T) => void): Promise<PluginListener>;
type PermissionState = 'granted' | 'denied' | 'prompt' | 'prompt-with-rationale';
/**
 * Get permission state for a plugin.
 *
 * This should be used by plugin authors to wrap their actual implementation.
 */
declare function checkPermissions<T>(plugin: string): Promise<T>;
/**
 * Request permissions.
 *
 * This should be used by plugin authors to wrap their actual implementation.
 */
declare function requestPermissions<T>(plugin: string): Promise<T>;
/**
 * Command arguments.
 *
 * @since 1.0.0
 */
type InvokeArgs = Record<string, unknown> | number[] | ArrayBuffer | Uint8Array;
/**
 * @since 2.0.0
 */
interface InvokeOptions {
    headers: HeadersInit;
}
/**
 * Sends a message to the backend.
 * @example
 * ```typescript
 * import { invoke } from '@tauri-apps/api/core';
 * await invoke('login', { user: 'tauri', password: 'poiwe3h4r5ip3yrhtew9ty' });
 * ```
 *
 * @param cmd The command name.
 * @param args The optional arguments to pass to the command.
 * @param options The request options.
 * @return A promise resolving or rejecting to the backend response.
 *
 * @since 1.0.0
 */
declare function invoke<T>(cmd: string, args?: InvokeArgs, options?: InvokeOptions): Promise<T>;
/**
 * Convert a device file path to an URL that can be loaded by the webview.
 * Note that `asset:` and `http://asset.localhost` must be added to [`app.security.csp`](https://v2.tauri.app/reference/config/#csp-1) in `tauri.conf.json`.
 * Example CSP value: `"csp": "default-src 'self' ipc: http://ipc.localhost; img-src 'self' asset: http://asset.localhost"` to use the asset protocol on image sources.
 *
 * Additionally, `"enable" : "true"` must be added to [`app.security.assetProtocol`](https://v2.tauri.app/reference/config/#assetprotocolconfig)
 * in `tauri.conf.json` and its access scope must be defined on the `scope` array on the same `assetProtocol` object.
 *
 * @param  filePath The file path.
 * @param  protocol The protocol to use. Defaults to `asset`. You only need to set this when using a custom protocol.
 * @example
 * ```typescript
 * import { appDataDir, join } from '@tauri-apps/api/path';
 * import { convertFileSrc } from '@tauri-apps/api/core';
 * const appDataDirPath = await appDataDir();
 * const filePath = await join(appDataDirPath, 'assets/video.mp4');
 * const assetUrl = convertFileSrc(filePath);
 *
 * const video = document.getElementById('my-video');
 * const source = document.createElement('source');
 * source.type = 'video/mp4';
 * source.src = assetUrl;
 * video.appendChild(source);
 * video.load();
 * ```
 *
 * @return the URL that can be used as source on the webview.
 *
 * @since 1.0.0
 */
declare function convertFileSrc(filePath: string, protocol?: string): string;
/**
 * A rust-backed resource stored through `tauri::Manager::resources_table` API.
 *
 * The resource lives in the main process and does not exist
 * in the Javascript world, and thus will not be cleaned up automatically
 * except on application exit. If you want to clean it up early, call {@linkcode Resource.close}
 *
 * @example
 * ```typescript
 * import { Resource, invoke } from '@tauri-apps/api/core';
 * export class DatabaseHandle extends Resource {
 *   static async open(path: string): Promise<DatabaseHandle> {
 *     const rid: number = await invoke('open_db', { path });
 *     return new DatabaseHandle(rid);
 *   }
 *
 *   async execute(sql: string): Promise<void> {
 *     await invoke('execute_sql', { rid: this.rid, sql });
 *   }
 * }
 * ```
 */
export declare class Resource {
    #private;
    get rid(): number;
    constructor(rid: number);
    /**
     * Destroys and cleans up this resource from memory.
     * **You should not call any method on this object anymore and should drop any reference to it.**
     */
    close(): Promise<void>;
}
declare function isTauri(): boolean;
export type { InvokeArgs, InvokeOptions };
export { transformCallback, Channel, PluginListener, addPluginListener, PermissionState, checkPermissions, requestPermissions, invoke, convertFileSrc, isTauri };
