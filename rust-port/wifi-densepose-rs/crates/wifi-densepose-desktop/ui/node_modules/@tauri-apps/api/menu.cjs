'use strict';

var submenu = require('./menu/submenu.cjs');
var menuItem = require('./menu/menuItem.cjs');
var menu = require('./menu/menu.cjs');
var checkMenuItem = require('./menu/checkMenuItem.cjs');
var iconMenuItem = require('./menu/iconMenuItem.cjs');
var predefinedMenuItem = require('./menu/predefinedMenuItem.cjs');

// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
/**
 * Menu types and utilities.
 *
 * This package is also accessible with `window.__TAURI__.menu` when [`app.withGlobalTauri`](https://v2.tauri.app/reference/config/#withglobaltauri) in `tauri.conf.json` is set to `true`.
 * @module
 */

exports.Submenu = submenu.Submenu;
exports.itemFromKind = submenu.itemFromKind;
exports.MenuItem = menuItem.MenuItem;
exports.Menu = menu.Menu;
exports.CheckMenuItem = checkMenuItem.CheckMenuItem;
exports.IconMenuItem = iconMenuItem.IconMenuItem;
Object.defineProperty(exports, "NativeIcon", {
	enumerable: true,
	get: function () { return iconMenuItem.NativeIcon; }
});
exports.PredefinedMenuItem = predefinedMenuItem.PredefinedMenuItem;
