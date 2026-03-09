import { IconMenuItemOptions, PredefinedMenuItemOptions, CheckMenuItemOptions } from '../menu';
import { MenuItem, type MenuItemOptions } from './menuItem';
import { CheckMenuItem } from './checkMenuItem';
import { IconMenuItem } from './iconMenuItem';
import { PredefinedMenuItem } from './predefinedMenuItem';
import { type LogicalPosition, PhysicalPosition, type Window } from '../window';
import { type ItemKind, MenuItemBase } from './base';
import { type MenuOptions } from './menu';
import { MenuIcon } from '../image';
/** @ignore */
export declare function itemFromKind([rid, id, kind]: [number, string, ItemKind]): Submenu | MenuItem | PredefinedMenuItem | CheckMenuItem | IconMenuItem;
export type SubmenuOptions = (Omit<MenuItemOptions, 'accelerator' | 'action'> & MenuOptions) & {
    /**
     * Icon to be used for the submenu.
     * Note: you may need the `image-ico` or `image-png` Cargo features to use this API.
     */
    icon?: MenuIcon;
};
/** A type that is a submenu inside a {@linkcode Menu} or {@linkcode Submenu}. */
export declare class Submenu extends MenuItemBase {
    /** @ignore */
    protected constructor(rid: number, id: string);
    /** Create a new submenu. */
    static new(opts: SubmenuOptions): Promise<Submenu>;
    /** Returns the text of this submenu. */
    text(): Promise<string>;
    /** Sets the text for this submenu. */
    setText(text: string): Promise<void>;
    /** Returns whether this submenu is enabled or not. */
    isEnabled(): Promise<boolean>;
    /** Sets whether this submenu is enabled or not. */
    setEnabled(enabled: boolean): Promise<void>;
    /**
     * Add a menu item to the end of this submenu.
     *
     * #### Platform-specific:
     *
     * - **macOS:** Only {@linkcode Submenu}s can be added to a {@linkcode Menu}.
     */
    append<T extends Submenu | MenuItem | PredefinedMenuItem | CheckMenuItem | IconMenuItem | MenuItemOptions | SubmenuOptions | IconMenuItemOptions | PredefinedMenuItemOptions | CheckMenuItemOptions>(items: T | T[]): Promise<void>;
    /**
     * Add a menu item to the beginning of this submenu.
     *
     * #### Platform-specific:
     *
     * - **macOS:** Only {@linkcode Submenu}s can be added to a {@linkcode Menu}.
     */
    prepend<T extends Submenu | MenuItem | PredefinedMenuItem | CheckMenuItem | IconMenuItem | MenuItemOptions | SubmenuOptions | IconMenuItemOptions | PredefinedMenuItemOptions | CheckMenuItemOptions>(items: T | T[]): Promise<void>;
    /**
     * Add a menu item to the specified position in this submenu.
     *
     * #### Platform-specific:
     *
     * - **macOS:** Only {@linkcode Submenu}s can be added to a {@linkcode Menu}.
     */
    insert<T extends Submenu | MenuItem | PredefinedMenuItem | CheckMenuItem | IconMenuItem | MenuItemOptions | SubmenuOptions | IconMenuItemOptions | PredefinedMenuItemOptions | CheckMenuItemOptions>(items: T | T[], position: number): Promise<void>;
    /** Remove a menu item from this submenu. */
    remove(item: Submenu | MenuItem | PredefinedMenuItem | CheckMenuItem | IconMenuItem): Promise<void>;
    /** Remove a menu item from this submenu at the specified position. */
    removeAt(position: number): Promise<Submenu | MenuItem | PredefinedMenuItem | CheckMenuItem | IconMenuItem | null>;
    /** Returns a list of menu items that has been added to this submenu. */
    items(): Promise<Array<Submenu | MenuItem | PredefinedMenuItem | CheckMenuItem | IconMenuItem>>;
    /** Retrieves the menu item matching the given identifier. */
    get(id: string): Promise<Submenu | MenuItem | PredefinedMenuItem | CheckMenuItem | IconMenuItem | null>;
    /**
     * Popup this submenu as a context menu on the specified window.
     *
     * If the position, is provided, it is relative to the window's top-left corner.
     */
    popup(at?: PhysicalPosition | LogicalPosition, window?: Window): Promise<void>;
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
    setAsWindowsMenuForNSApp(): Promise<void>;
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
    setAsHelpMenuForNSApp(): Promise<void>;
    /** Sets an icon for this submenu */
    setIcon(icon: MenuIcon | null): Promise<void>;
}
