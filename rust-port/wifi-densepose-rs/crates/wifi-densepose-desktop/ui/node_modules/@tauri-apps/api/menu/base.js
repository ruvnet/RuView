import { __classPrivateFieldGet, __classPrivateFieldSet } from '../external/tslib/tslib.es6.js';
import { Resource, Channel, invoke } from '../core.js';
import { transformImage } from '../image.js';

// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
var _MenuItemBase_id, _MenuItemBase_kind;
function injectChannel(i) {
    var _a;
    if ('items' in i) {
        i.items = (_a = i.items) === null || _a === void 0 ? void 0 : _a.map((item) => 'rid' in item ? item : injectChannel(item));
    }
    else if ('action' in i && i.action) {
        const handler = new Channel();
        handler.onmessage = i.action;
        delete i.action;
        return { ...i, handler };
    }
    return i;
}
async function newMenu(kind, opts) {
    const handler = new Channel();
    if (opts && typeof opts === 'object') {
        if ('action' in opts && opts.action) {
            handler.onmessage = opts.action;
            delete opts.action;
        }
        // about predefined menu item icon
        if ('item' in opts
            && opts.item
            && typeof opts.item === 'object'
            && 'About' in opts.item
            && opts.item.About
            && typeof opts.item.About === 'object'
            && 'icon' in opts.item.About
            && opts.item.About.icon) {
            opts.item.About.icon = transformImage(opts.item.About.icon);
        }
        // icon menu item icon
        if ('icon' in opts && opts.icon) {
            opts.icon = transformImage(opts.icon);
        }
        // submenu items
        if ('items' in opts && opts.items) {
            function prepareItem(i) {
                var _a;
                if ('rid' in i) {
                    return [i.rid, i.kind];
                }
                if ('item' in i && typeof i.item === 'object' && ((_a = i.item.About) === null || _a === void 0 ? void 0 : _a.icon)) {
                    i.item.About.icon = transformImage(i.item.About.icon);
                }
                if ('icon' in i && i.icon) {
                    i.icon = transformImage(i.icon);
                }
                if ('items' in i && i.items) {
                    // @ts-expect-error the `prepareItem` return doesn't exactly match
                    // this is fine, because the difference is in `[number, string]` variant
                    i.items = i.items.map(prepareItem);
                }
                return injectChannel(i);
            }
            // @ts-expect-error the `prepareItem` return doesn't exactly match
            // this is fine, because the difference is in `[number, string]` variant
            opts.items = opts.items.map(prepareItem);
        }
    }
    return invoke('plugin:menu|new', {
        kind,
        options: opts,
        handler
    });
}
class MenuItemBase extends Resource {
    /** The id of this item. */
    get id() {
        return __classPrivateFieldGet(this, _MenuItemBase_id, "f");
    }
    /** @ignore */
    get kind() {
        return __classPrivateFieldGet(this, _MenuItemBase_kind, "f");
    }
    /** @ignore */
    constructor(rid, id, kind) {
        super(rid);
        /** @ignore */
        _MenuItemBase_id.set(this, void 0);
        /** @ignore */
        _MenuItemBase_kind.set(this, void 0);
        __classPrivateFieldSet(this, _MenuItemBase_id, id, "f");
        __classPrivateFieldSet(this, _MenuItemBase_kind, kind, "f");
    }
}
_MenuItemBase_id = new WeakMap(), _MenuItemBase_kind = new WeakMap();

export { MenuItemBase, newMenu };
