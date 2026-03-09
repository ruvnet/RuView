import { Resource, invoke, Channel } from './core.js';
import { transformImage } from './image.js';
import { PhysicalPosition, PhysicalSize } from './dpi.js';

// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
/**
 * Tray icon class and associated methods. This type constructor is private,
 * instead, you should use the static method {@linkcode TrayIcon.new}.
 *
 * #### Warning
 *
 * Unlike Rust, javascript does not have any way to run cleanup code
 * when an object is being removed by garbage collection, but this tray icon
 * will be cleaned up when the tauri app exists, however if you want to cleanup
 * this object early, you need to call {@linkcode TrayIcon.close}.
 *
 * @example
 * ```ts
 * import { TrayIcon } from '@tauri-apps/api/tray';
 * const tray = await TrayIcon.new({ tooltip: 'awesome tray tooltip' });
 * tray.set_tooltip('new tooltip');
 * ```
 */
class TrayIcon extends Resource {
    constructor(rid, id) {
        super(rid);
        this.id = id;
    }
    /** Gets a tray icon using the provided id. */
    static async getById(id) {
        return invoke('plugin:tray|get_by_id', { id }).then((rid) => rid ? new TrayIcon(rid, id) : null);
    }
    /**
     * Removes a tray icon using the provided id from tauri's internal state.
     *
     * Note that this may cause the tray icon to disappear
     * if it wasn't cloned somewhere else or referenced by JS.
     */
    static async removeById(id) {
        return invoke('plugin:tray|remove_by_id', { id });
    }
    /**
     * Creates a new {@linkcode TrayIcon}
     *
     * #### Platform-specific:
     *
     * - **Linux:** Sometimes the icon won't be visible unless a menu is set.
     * Setting an empty {@linkcode Menu} is enough.
     */
    static async new(options) {
        if (options === null || options === void 0 ? void 0 : options.menu) {
            // @ts-expect-error we only need the rid and kind
            options.menu = [options.menu.rid, options.menu.kind];
        }
        if (options === null || options === void 0 ? void 0 : options.icon) {
            options.icon = transformImage(options.icon);
        }
        const handler = new Channel();
        if (options === null || options === void 0 ? void 0 : options.action) {
            const action = options.action;
            handler.onmessage = (e) => action(mapEvent(e));
            delete options.action;
        }
        return invoke('plugin:tray|new', {
            options: options !== null && options !== void 0 ? options : {},
            handler
        }).then(([rid, id]) => new TrayIcon(rid, id));
    }
    /**
     *  Sets a new tray icon. If `null` is provided, it will remove the icon.
     *
     * Note that you may need the `image-ico` or `image-png` Cargo features to use this API.
     * To enable it, change your Cargo.toml file:
     * ```toml
     * [dependencies]
     * tauri = { version = "...", features = ["...", "image-png"] }
     * ```
     */
    async setIcon(icon) {
        let trayIcon = null;
        if (icon) {
            trayIcon = transformImage(icon);
        }
        return invoke('plugin:tray|set_icon', { rid: this.rid, icon: trayIcon });
    }
    /**
     * Sets a new tray menu.
     *
     * #### Platform-specific:
     *
     * - **Linux**: once a menu is set it cannot be removed so `null` has no effect
     */
    async setMenu(menu) {
        if (menu) {
            // @ts-expect-error we only need the rid and kind
            menu = [menu.rid, menu.kind];
        }
        return invoke('plugin:tray|set_menu', { rid: this.rid, menu });
    }
    /**
     * Sets the tooltip for this tray icon.
     *
     * #### Platform-specific:
     *
     * - **Linux:** Unsupported
     */
    async setTooltip(tooltip) {
        return invoke('plugin:tray|set_tooltip', { rid: this.rid, tooltip });
    }
    /**
     * Sets the tooltip for this tray icon.
     *
     * #### Platform-specific:
     *
     * - **Linux:** The title will not be shown unless there is an icon
     * as well.  The title is useful for numerical and other frequently
     * updated information.  In general, it shouldn't be shown unless a
     * user requests it as it can take up a significant amount of space
     * on the user's panel.  This may not be shown in all visualizations.
     * - **Windows:** Unsupported
     */
    async setTitle(title) {
        return invoke('plugin:tray|set_title', { rid: this.rid, title });
    }
    /** Show or hide this tray icon. */
    async setVisible(visible) {
        return invoke('plugin:tray|set_visible', { rid: this.rid, visible });
    }
    /**
     * Sets the tray icon temp dir path. **Linux only**.
     *
     * On Linux, we need to write the icon to the disk and usually it will
     * be `$XDG_RUNTIME_DIR/tray-icon` or `$TEMP/tray-icon`.
     */
    async setTempDirPath(path) {
        return invoke('plugin:tray|set_temp_dir_path', { rid: this.rid, path });
    }
    /** Sets the current icon as a [template](https://developer.apple.com/documentation/appkit/nsimage/1520017-template?language=objc). **macOS only** */
    async setIconAsTemplate(asTemplate) {
        return invoke('plugin:tray|set_icon_as_template', {
            rid: this.rid,
            asTemplate
        });
    }
    /**
     *  Disable or enable showing the tray menu on left click.
     *
     * #### Platform-specific:
     *
     * - **Linux**: Unsupported.
     *
     * @deprecated use {@linkcode TrayIcon.setShowMenuOnLeftClick} instead.
     */
    async setMenuOnLeftClick(onLeft) {
        return invoke('plugin:tray|set_show_menu_on_left_click', {
            rid: this.rid,
            onLeft
        });
    }
    /**
     *  Disable or enable showing the tray menu on left click.
     *
     * #### Platform-specific:
     *
     * - **Linux**: Unsupported.
     *
     * @since 2.2.0
     */
    async setShowMenuOnLeftClick(onLeft) {
        return invoke('plugin:tray|set_show_menu_on_left_click', {
            rid: this.rid,
            onLeft
        });
    }
}
function mapEvent(e) {
    const out = e;
    out.position = new PhysicalPosition(e.position);
    out.rect.position = new PhysicalPosition(e.rect.position);
    out.rect.size = new PhysicalSize(e.rect.size);
    return out;
}

export { TrayIcon };
