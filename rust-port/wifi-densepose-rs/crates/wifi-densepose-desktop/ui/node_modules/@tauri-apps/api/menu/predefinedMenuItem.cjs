'use strict';

var base = require('./base.cjs');
var core = require('../core.cjs');

// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
/** A predefined (native) menu item which has a predefined behavior by the OS or by tauri.  */
class PredefinedMenuItem extends base.MenuItemBase {
    /** @ignore */
    constructor(rid, id) {
        super(rid, id, 'Predefined');
    }
    /** Create a new predefined menu item. */
    static async new(opts) {
        return base.newMenu('Predefined', opts).then(([rid, id]) => new PredefinedMenuItem(rid, id));
    }
    /** Returns the text of this predefined menu item. */
    async text() {
        return core.invoke('plugin:menu|text', { rid: this.rid, kind: this.kind });
    }
    /** Sets the text for this predefined menu item. */
    async setText(text) {
        return core.invoke('plugin:menu|set_text', {
            rid: this.rid,
            kind: this.kind,
            text
        });
    }
}

exports.PredefinedMenuItem = PredefinedMenuItem;
