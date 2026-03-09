'use strict';

var core = require('./core.cjs');

// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
/** An RGBA Image in row-major order from top to bottom. */
class Image extends core.Resource {
    /**
     * Creates an Image from a resource ID. For internal use only.
     *
     * @ignore
     */
    constructor(rid) {
        super(rid);
    }
    /** Creates a new Image using RGBA data, in row-major order from top to bottom, and with specified width and height. */
    static async new(rgba, width, height) {
        return core.invoke('plugin:image|new', {
            rgba: transformImage(rgba),
            width,
            height
        }).then((rid) => new Image(rid));
    }
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
    static async fromBytes(bytes) {
        return core.invoke('plugin:image|from_bytes', {
            bytes: transformImage(bytes)
        }).then((rid) => new Image(rid));
    }
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
    static async fromPath(path) {
        return core.invoke('plugin:image|from_path', { path }).then((rid) => new Image(rid));
    }
    /** Returns the RGBA data for this image, in row-major order from top to bottom.  */
    async rgba() {
        return core.invoke('plugin:image|rgba', {
            rid: this.rid
        }).then((buffer) => new Uint8Array(buffer));
    }
    /** Returns the size of this image.  */
    async size() {
        return core.invoke('plugin:image|size', { rid: this.rid });
    }
}
/**
 * Transforms image from various types into a type acceptable by Rust.
 *
 * See [tauri::image::JsImage](https://docs.rs/tauri/2/tauri/image/enum.JsImage.html) for more information.
 * Note the API signature is not stable and might change.
 */
function transformImage(image) {
    const ret = image == null
        ? null
        : typeof image === 'string'
            ? image
            : image instanceof Image
                ? image.rid
                : image;
    return ret;
}

exports.Image = Image;
exports.transformImage = transformImage;
