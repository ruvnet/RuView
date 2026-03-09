import { MenuItemBase, newMenu } from './base.js';
import { invoke } from '../core.js';
import { transformImage } from '../image.js';

// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
/**
 * A native Icon to be used for the menu item
 *
 * #### Platform-specific:
 *
 * - **Windows / Linux**: Unsupported.
 */
var NativeIcon;
(function (NativeIcon) {
    /** An add item template image. */
    NativeIcon["Add"] = "Add";
    /** Advanced preferences toolbar icon for the preferences window. */
    NativeIcon["Advanced"] = "Advanced";
    /** A Bluetooth template image. */
    NativeIcon["Bluetooth"] = "Bluetooth";
    /** Bookmarks image suitable for a template. */
    NativeIcon["Bookmarks"] = "Bookmarks";
    /** A caution image. */
    NativeIcon["Caution"] = "Caution";
    /** A color panel toolbar icon. */
    NativeIcon["ColorPanel"] = "ColorPanel";
    /** A column view mode template image. */
    NativeIcon["ColumnView"] = "ColumnView";
    /** A computer icon. */
    NativeIcon["Computer"] = "Computer";
    /** An enter full-screen mode template image. */
    NativeIcon["EnterFullScreen"] = "EnterFullScreen";
    /** Permissions for all users. */
    NativeIcon["Everyone"] = "Everyone";
    /** An exit full-screen mode template image. */
    NativeIcon["ExitFullScreen"] = "ExitFullScreen";
    /** A cover flow view mode template image. */
    NativeIcon["FlowView"] = "FlowView";
    /** A folder image. */
    NativeIcon["Folder"] = "Folder";
    /** A burnable folder icon. */
    NativeIcon["FolderBurnable"] = "FolderBurnable";
    /** A smart folder icon. */
    NativeIcon["FolderSmart"] = "FolderSmart";
    /** A link template image. */
    NativeIcon["FollowLinkFreestanding"] = "FollowLinkFreestanding";
    /** A font panel toolbar icon. */
    NativeIcon["FontPanel"] = "FontPanel";
    /** A `go back` template image. */
    NativeIcon["GoLeft"] = "GoLeft";
    /** A `go forward` template image. */
    NativeIcon["GoRight"] = "GoRight";
    /** Home image suitable for a template. */
    NativeIcon["Home"] = "Home";
    /** An iChat Theater template image. */
    NativeIcon["IChatTheater"] = "IChatTheater";
    /** An icon view mode template image. */
    NativeIcon["IconView"] = "IconView";
    /** An information toolbar icon. */
    NativeIcon["Info"] = "Info";
    /** A template image used to denote invalid data. */
    NativeIcon["InvalidDataFreestanding"] = "InvalidDataFreestanding";
    /** A generic left-facing triangle template image. */
    NativeIcon["LeftFacingTriangle"] = "LeftFacingTriangle";
    /** A list view mode template image. */
    NativeIcon["ListView"] = "ListView";
    /** A locked padlock template image. */
    NativeIcon["LockLocked"] = "LockLocked";
    /** An unlocked padlock template image. */
    NativeIcon["LockUnlocked"] = "LockUnlocked";
    /** A horizontal dash, for use in menus. */
    NativeIcon["MenuMixedState"] = "MenuMixedState";
    /** A check mark template image, for use in menus. */
    NativeIcon["MenuOnState"] = "MenuOnState";
    /** A MobileMe icon. */
    NativeIcon["MobileMe"] = "MobileMe";
    /** A drag image for multiple items. */
    NativeIcon["MultipleDocuments"] = "MultipleDocuments";
    /** A network icon. */
    NativeIcon["Network"] = "Network";
    /** A path button template image. */
    NativeIcon["Path"] = "Path";
    /** General preferences toolbar icon for the preferences window. */
    NativeIcon["PreferencesGeneral"] = "PreferencesGeneral";
    /** A Quick Look template image. */
    NativeIcon["QuickLook"] = "QuickLook";
    /** A refresh template image. */
    NativeIcon["RefreshFreestanding"] = "RefreshFreestanding";
    /** A refresh template image. */
    NativeIcon["Refresh"] = "Refresh";
    /** A remove item template image. */
    NativeIcon["Remove"] = "Remove";
    /** A reveal contents template image. */
    NativeIcon["RevealFreestanding"] = "RevealFreestanding";
    /** A generic right-facing triangle template image. */
    NativeIcon["RightFacingTriangle"] = "RightFacingTriangle";
    /** A share view template image. */
    NativeIcon["Share"] = "Share";
    /** A slideshow template image. */
    NativeIcon["Slideshow"] = "Slideshow";
    /** A badge for a `smart` item. */
    NativeIcon["SmartBadge"] = "SmartBadge";
    /** Small green indicator, similar to iChat's available image. */
    NativeIcon["StatusAvailable"] = "StatusAvailable";
    /** Small clear indicator. */
    NativeIcon["StatusNone"] = "StatusNone";
    /** Small yellow indicator, similar to iChat's idle image. */
    NativeIcon["StatusPartiallyAvailable"] = "StatusPartiallyAvailable";
    /** Small red indicator, similar to iChat's unavailable image. */
    NativeIcon["StatusUnavailable"] = "StatusUnavailable";
    /** A stop progress template image. */
    NativeIcon["StopProgressFreestanding"] = "StopProgressFreestanding";
    /** A stop progress button template image. */
    NativeIcon["StopProgress"] = "StopProgress";
    /** An image of the empty trash can. */
    NativeIcon["TrashEmpty"] = "TrashEmpty";
    /** An image of the full trash can. */
    NativeIcon["TrashFull"] = "TrashFull";
    /** Permissions for a single user. */
    NativeIcon["User"] = "User";
    /** User account toolbar icon for the preferences window. */
    NativeIcon["UserAccounts"] = "UserAccounts";
    /** Permissions for a group of users. */
    NativeIcon["UserGroup"] = "UserGroup";
    /** Permissions for guests. */
    NativeIcon["UserGuest"] = "UserGuest";
})(NativeIcon || (NativeIcon = {}));
/**
 * An icon menu item inside a {@linkcode Menu} or {@linkcode Submenu}
 * and usually contains an icon and a text.
 */
class IconMenuItem extends MenuItemBase {
    /** @ignore */
    constructor(rid, id) {
        super(rid, id, 'Icon');
    }
    /** Create a new icon menu item. */
    static async new(opts) {
        return newMenu('Icon', opts).then(([rid, id]) => new IconMenuItem(rid, id));
    }
    /** Returns the text of this icon menu item. */
    async text() {
        return invoke('plugin:menu|text', { rid: this.rid, kind: this.kind });
    }
    /** Sets the text for this icon menu item. */
    async setText(text) {
        return invoke('plugin:menu|set_text', {
            rid: this.rid,
            kind: this.kind,
            text
        });
    }
    /** Returns whether this icon menu item is enabled or not. */
    async isEnabled() {
        return invoke('plugin:menu|is_enabled', { rid: this.rid, kind: this.kind });
    }
    /** Sets whether this icon menu item is enabled or not. */
    async setEnabled(enabled) {
        return invoke('plugin:menu|set_enabled', {
            rid: this.rid,
            kind: this.kind,
            enabled
        });
    }
    /** Sets the accelerator for this icon menu item. */
    async setAccelerator(accelerator) {
        return invoke('plugin:menu|set_accelerator', {
            rid: this.rid,
            kind: this.kind,
            accelerator
        });
    }
    /** Sets an icon for this icon menu item */
    async setIcon(icon) {
        return invoke('plugin:menu|set_icon', {
            rid: this.rid,
            kind: this.kind,
            icon: transformImage(icon)
        });
    }
}

export { IconMenuItem, NativeIcon };
