import { MenuItemOptions, SubmenuOptions, IconMenuItemOptions, PredefinedMenuItemOptions, CheckMenuItemOptions } from '../menu';
import { MenuItem } from './menuItem';
import { CheckMenuItem } from './checkMenuItem';
import { IconMenuItem } from './iconMenuItem';
import { PredefinedMenuItem } from './predefinedMenuItem';
import { Submenu } from './submenu';
import { type LogicalPosition, PhysicalPosition, Position } from '../dpi';
import { type Window } from '../window';
import { MenuItemBase } from './base';
/** Options for creating a new menu. */
export interface MenuOptions {
    /** Specify an id to use for the new menu. */
    id?: string;
    /** List of items to add to the new menu. */
    items?: Array<Submenu | MenuItem | PredefinedMenuItem | CheckMenuItem | IconMenuItem | MenuItemOptions | SubmenuOptions | IconMenuItemOptions | PredefinedMenuItemOptions | CheckMenuItemOptions>;
}
/** A type that is either a menu bar on the window
 * on Windows and Linux or as a global menu in the menubar on macOS.
 *
 * #### Platform-specific:
 *
 * - **macOS**: if using {@linkcode Menu} for the global menubar, it can only contain {@linkcode Submenu}s.
 */
export declare class Menu extends MenuItemBase {
    /** @ignore */
    protected constructor(rid: number, id: string);
    /** Create a new menu. */
    static new(opts?: MenuOptions): Promise<Menu>;
    /** Create a default menu. */
    static default(): Promise<Menu>;
    /**
     * Add a menu item to the end of this menu.
     *
     * #### Platform-specific:
     *
     * - **macOS:** Only {@linkcode Submenu}s can be added to a {@linkcode Menu}.
     */
    append<T extends Submenu | MenuItem | PredefinedMenuItem | CheckMenuItem | IconMenuItem | MenuItemOptions | SubmenuOptions | IconMenuItemOptions | PredefinedMenuItemOptions | CheckMenuItemOptions>(items: T | T[]): Promise<void>;
    /**
     * Add a menu item to the beginning of this menu.
     *
     * #### Platform-specific:
     *
     * - **macOS:** Only {@linkcode Submenu}s can be added to a {@linkcode Menu}.
     */
    prepend<T extends Submenu | MenuItem | PredefinedMenuItem | CheckMenuItem | IconMenuItem | MenuItemOptions | SubmenuOptions | IconMenuItemOptions | PredefinedMenuItemOptions | CheckMenuItemOptions>(items: T | T[]): Promise<void>;
    /**
     * Add a menu item to the specified position in this menu.
     *
     * #### Platform-specific:
     *
     * - **macOS:** Only {@linkcode Submenu}s can be added to a {@linkcode Menu}.
     */
    insert<T extends Submenu | MenuItem | PredefinedMenuItem | CheckMenuItem | IconMenuItem | MenuItemOptions | SubmenuOptions | IconMenuItemOptions | PredefinedMenuItemOptions | CheckMenuItemOptions>(items: T | T[], position: number): Promise<void>;
    /** Remove a menu item from this menu. */
    remove(item: Submenu | MenuItem | PredefinedMenuItem | CheckMenuItem | IconMenuItem): Promise<void>;
    /** Remove a menu item from this menu at the specified position. */
    removeAt(position: number): Promise<Submenu | MenuItem | PredefinedMenuItem | CheckMenuItem | IconMenuItem | null>;
    /** Returns a list of menu items that has been added to this menu. */
    items(): Promise<Array<Submenu | MenuItem | PredefinedMenuItem | CheckMenuItem | IconMenuItem>>;
    /** Retrieves the menu item matching the given identifier. */
    get(id: string): Promise<Submenu | MenuItem | PredefinedMenuItem | CheckMenuItem | IconMenuItem | null>;
    /**
     * Popup this menu as a context menu on the specified window.
     *
     * @param at If a position is provided, it is relative to the window's top-left corner.
     * If there isn't one provided, the menu will pop up at the current location of the mouse.
     */
    popup(at?: PhysicalPosition | LogicalPosition | Position, window?: Window): Promise<void>;
    /**
     * Sets the app-wide menu and returns the previous one.
     *
     * If a window was not created with an explicit menu or had one set explicitly,
     * this menu will be assigned to it.
     */
    setAsAppMenu(): Promise<Menu | null>;
    /**
     * Sets the window menu and returns the previous one.
     *
     * #### Platform-specific:
     *
     * - **macOS:** Unsupported. The menu on macOS is app-wide and not specific to one
     * window, if you need to set it, use {@linkcode Menu.setAsAppMenu} instead.
     */
    setAsWindowMenu(window?: Window): Promise<Menu | null>;
}
