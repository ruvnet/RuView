'use strict';

var menuItem = require('./menuItem.cjs');
var checkMenuItem = require('./checkMenuItem.cjs');
var iconMenuItem = require('./iconMenuItem.cjs');
var predefinedMenuItem = require('./predefinedMenuItem.cjs');
var core = require('../core.cjs');
var base = require('./base.cjs');
var dpi = require('../dpi.cjs');
var image = require('../image.cjs');

// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
/** @ignore */
function itemFromKind([rid, id, kind]) {
    /* eslint-disable @typescript-eslint/no-unsafe-return */
    switch (kind) {
        case 'Submenu':
            // @ts-expect-error constructor is protected for external usage only, safe for us to use
            return new Submenu(rid, id);
        case 'Predefined':
            // @ts-expect-error constructor is protected for external usage only, safe for us to use
            return new predefinedMenuItem.PredefinedMenuItem(rid, id);
        case 'Check':
            // @ts-expect-error constructor is protected for external usage only, safe for us to use
            return new checkMenuItem.CheckMenuItem(rid, id);
        case 'Icon':
            // @ts-expect-error constructor is protected for external usage only, safe for us to use
            return new iconMenuItem.IconMenuItem(rid, id);
        case 'MenuItem':
        default:
            // @ts-expect-error constructor is protected for external usage only, safe for us to use
            return new menuItem.MenuItem(rid, id);
    }
    /* eslint-enable @typescript-eslint/no-unsafe-return */
}
/** A type that is a submenu inside a {@linkcode Menu} or {@linkcode Submenu}. */
class Submenu extends base.MenuItemBase {
    /** @ignore */
    constructor(rid, id) {
        super(rid, id, 'Submenu');
    }
    /** Create a new submenu. */
    static async new(opts) {
        return base.newMenu('Submenu', opts).then(([rid, id]) => new Submenu(rid, id));
    }
    /** Returns the text of this submenu. */
    async text() {
        return core.invoke('plugin:menu|text', { rid: this.rid, kind: this.kind });
    }
    /** Sets the text for this submenu. */
    async setText(text) {
        return core.invoke('plugin:menu|set_text', {
            rid: this.rid,
            kind: this.kind,
            text
        });
    }
    /** Returns whether this submenu is enabled or not. */
    async isEnabled() {
        return core.invoke('plugin:menu|is_enabled', { rid: this.rid, kind: this.kind });
    }
    /** Sets whether this submenu is enabled or not. */
    async setEnabled(enabled) {
        return core.invoke('plugin:menu|set_enabled', {
            rid: this.rid,
            kind: this.kind,
            enabled
        });
    }
    /**
     * Add a menu item to the end of this submenu.
     *
     * #### Platform-specific:
     *
     * - **macOS:** Only {@linkcode Submenu}s can be added to a {@linkcode Menu}.
     */
    async append(items) {
        return core.invoke('plugin:menu|append', {
            rid: this.rid,
            kind: this.kind,
            items: (Array.isArray(items) ? items : [items]).map((i) => 'rid' in i ? [i.rid, i.kind] : i)
        });
    }
    /**
     * Add a menu item to the beginning of this submenu.
     *
     * #### Platform-specific:
     *
     * - **macOS:** Only {@linkcode Submenu}s can be added to a {@linkcode Menu}.
     */
    async prepend(items) {
        return core.invoke('plugin:menu|prepend', {
            rid: this.rid,
            kind: this.kind,
            items: (Array.isArray(items) ? items : [items]).map((i) => 'rid' in i ? [i.rid, i.kind] : i)
        });
    }
    /**
     * Add a menu item to the specified position in this submenu.
     *
     * #### Platform-specific:
     *
     * - **macOS:** Only {@linkcode Submenu}s can be added to a {@linkcode Menu}.
     */
    async insert(items, position) {
        return core.invoke('plugin:menu|insert', {
            rid: this.rid,
            kind: this.kind,
            items: (Array.isArray(items) ? items : [items]).map((i) => 'rid' in i ? [i.rid, i.kind] : i),
            position
        });
    }
    /** Remove a menu item from this submenu. */
    async remove(item) {
        return core.invoke('plugin:menu|remove', {
            rid: this.rid,
            kind: this.kind,
            item: [item.rid, item.kind]
        });
    }
    /** Remove a menu item from this submenu at the specified position. */
    async removeAt(position) {
        return core.invoke('plugin:menu|remove_at', {
            rid: this.rid,
            kind: this.kind,
            position
        }).then(itemFromKind);
    }
    /** Returns a list of menu items that has been added to this submenu. */
    async items() {
        return core.invoke('plugin:menu|items', {
            rid: this.rid,
            kind: this.kind
        }).then((i) => i.map(itemFromKind));
    }
    /** Retrieves the menu item matching the given identifier. */
    async get(id) {
        return core.invoke('plugin:menu|get', {
            rid: this.rid,
            kind: this.kind,
            id
        }).then((r) => (r ? itemFromKind(r) : null));
    }
    /**
     * Popup this submenu as a context menu on the specified window.
     *
     * If the position, is provided, it is relative to the window's top-left corner.
     */
    async popup(at, window) {
        var _a;
        return core.invoke('plugin:menu|popup', {
            rid: this.rid,
            kind: this.kind,
            window: (_a = window === null || window === void 0 ? void 0 : window.label) !== null && _a !== void 0 ? _a : null,
            at: at instanceof dpi.Position ? at : at ? new dpi.Position(at) : null
        });
    }
    /**
     * Set this submenu as the Window menu for the application on macOS.
     *
     * This will cause macOS to automatically add window-switching items and
     * certain other items to the menu.
     *
     * #### Platform-specific:
     *
     * - **Windows / Linux**: Unsupported.
     */
    async setAsWindowsMenuForNSApp() {
        return core.invoke('plugin:menu|set_as_windows_menu_for_nsapp', {
            rid: this.rid
        });
    }
    /**
     * Set this submenu as the Help menu for the application on macOS.
     *
     * This will cause macOS to automatically add a search box to the menu.
     *
     * If no menu is set as the Help menu, macOS will automatically use any menu
     * which has a title matching the localized word "Help".
     *
     * #### Platform-specific:
     *
     * - **Windows / Linux**: Unsupported.
     */
    async setAsHelpMenuForNSApp() {
        return core.invoke('plugin:menu|set_as_help_menu_for_nsapp', {
            rid: this.rid
        });
    }
    /** Sets an icon for this submenu */
    async setIcon(icon) {
        return core.invoke('plugin:menu|set_icon', {
            rid: this.rid,
            kind: this.kind,
            icon: image.transformImage(icon)
        });
    }
}

exports.Submenu = Submenu;
exports.itemFromKind = itemFromKind;
