'use strict';

var base = require('./base.cjs');
var core = require('../core.cjs');

// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
/** A menu item inside a {@linkcode Menu} or {@linkcode Submenu} and contains only text. */
class MenuItem extends base.MenuItemBase {
    /** @ignore */
    constructor(rid, id) {
        super(rid, id, 'MenuItem');
    }
    /** Create a new menu item. */
    static async new(opts) {
        return base.newMenu('MenuItem', opts).then(([rid, id]) => new MenuItem(rid, id));
    }
    /** Returns the text of this menu item. */
    async text() {
        return core.invoke('plugin:menu|text', { rid: this.rid, kind: this.kind });
    }
    /** Sets the text for this menu item. */
    async setText(text) {
        return core.invoke('plugin:menu|set_text', {
            rid: this.rid,
            kind: this.kind,
            text
        });
    }
    /** Returns whether this menu item is enabled or not. */
    async isEnabled() {
        return core.invoke('plugin:menu|is_enabled', { rid: this.rid, kind: this.kind });
    }
    /** Sets whether this menu item is enabled or not. */
    async setEnabled(enabled) {
        return core.invoke('plugin:menu|set_enabled', {
            rid: this.rid,
            kind: this.kind,
            enabled
        });
    }
    /** Sets the accelerator for this menu item. */
    async setAccelerator(accelerator) {
        return core.invoke('plugin:menu|set_accelerator', {
            rid: this.rid,
            kind: this.kind,
            accelerator
        });
    }
}

exports.MenuItem = MenuItem;
