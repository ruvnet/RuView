import { MenuItemBase, newMenu } from './base.js';
import { invoke } from '../core.js';

// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
/**
 * A check menu item inside a {@linkcode Menu} or {@linkcode Submenu}
 * and usually contains a text and a check mark or a similar toggle
 * that corresponds to a checked and unchecked states.
 */
class CheckMenuItem extends MenuItemBase {
    /** @ignore */
    constructor(rid, id) {
        super(rid, id, 'Check');
    }
    /** Create a new check menu item. */
    static async new(opts) {
        return newMenu('Check', opts).then(([rid, id]) => new CheckMenuItem(rid, id));
    }
    /** Returns the text of this check menu item. */
    async text() {
        return invoke('plugin:menu|text', { rid: this.rid, kind: this.kind });
    }
    /** Sets the text for this check menu item. */
    async setText(text) {
        return invoke('plugin:menu|set_text', {
            rid: this.rid,
            kind: this.kind,
            text
        });
    }
    /** Returns whether this check menu item is enabled or not. */
    async isEnabled() {
        return invoke('plugin:menu|is_enabled', { rid: this.rid, kind: this.kind });
    }
    /** Sets whether this check menu item is enabled or not. */
    async setEnabled(enabled) {
        return invoke('plugin:menu|set_enabled', {
            rid: this.rid,
            kind: this.kind,
            enabled
        });
    }
    /** Sets the accelerator for this check menu item. */
    async setAccelerator(accelerator) {
        return invoke('plugin:menu|set_accelerator', {
            rid: this.rid,
            kind: this.kind,
            accelerator
        });
    }
    /** Returns whether this check menu item is checked or not. */
    async isChecked() {
        return invoke('plugin:menu|is_checked', { rid: this.rid });
    }
    /** Sets whether this check menu item is checked or not. */
    async setChecked(checked) {
        return invoke('plugin:menu|set_checked', {
            rid: this.rid,
            checked
        });
    }
}

export { CheckMenuItem };
