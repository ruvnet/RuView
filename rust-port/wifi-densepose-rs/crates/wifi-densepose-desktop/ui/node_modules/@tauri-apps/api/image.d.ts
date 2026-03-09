import { Resource } from './core';
import { NativeIcon } from './menu/iconMenuItem';
export interface ImageSize {
    width: number;
    height: number;
}
/** A type that represents an icon that can be used in menu items. */
export type MenuIcon = NativeIcon | string | Image | Uint8Array | ArrayBuffer | number[];
/** An RGBA Image in row-major order from top to bottom. */
export declare class Image extends Resource {
    /**
     * Creates an Image from a resource ID. For internal use only.
     *
     * @ignore
     */
    constructor(rid: number);
    /** Creates a new Image using RGBA data, in row-major order from top to bottom, and with specified width and height. */
    static new(rgba: number[] | Uint8Array | ArrayBuffer, width: number, height: number): Promise<Image>;
    /**
     * Creates a new image using the provided bytes by inferring the file format.
     * If the format is known, prefer [@link Image.fromPngBytes] or [@link Image.fromIcoBytes].
     *
     * Only `ico` and `png` are supported (based on activated feature flag).
     *
     * Note that you need the `image-ico` or `image-png` Cargo features to use this API.
     * To enable it, change your Cargo.toml file:
     * ```toml
     * [dependencies]
     * tauri = { version = "...", features = ["...", "image-png"] }
     * ```
     */
    static fromBytes(bytes: number[] | Uint8Array | ArrayBuffer): Promise<Image>;
    /**
     * Creates a new image using the provided path.
     *
     * Only `ico` and `png` are supported (based on activated feature flag).
     *
     * Note that you need the `image-ico` or `image-png` Cargo features to use this API.
     * To enable it, change your Cargo.toml file:
     * ```toml
     * [dependencies]
     * tauri = { version = "...", features = ["...", "image-png"] }
     * ```
     */
    static fromPath(path: string): Promise<Image>;
    /** Returns the RGBA data for this image, in row-major order from top to bottom.  */
    rgba(): Promise<Uint8Array>;
    /** Returns the size of this image.  */
    size(): Promise<ImageSize>;
}
/**
 * Transforms image from various types into a type acceptable by Rust.
 *
 * See [tauri::image::JsImage](https://docs.rs/tauri/2/tauri/image/enum.JsImage.html) for more information.
 * Note the API signature is not stable and might change.
 */
export declare function transformImage<T>(image: string | Image | Uint8Array | ArrayBuffer | number[] | null): T;
