import { Resource } from '../core';
import { CheckMenuItemOptions } from './checkMenuItem';
import { IconMenuItemOptions } from './iconMenuItem';
import { MenuOptions } from './menu';
import { MenuItemOptions } from './menuItem';
import { PredefinedMenuItemOptions } from './predefinedMenuItem';
import { SubmenuOptions } from './submenu';
export type ItemKind = 'MenuItem' | 'Predefined' | 'Check' | 'Icon' | 'Submenu' | 'Menu';
export declare function newMenu(kind: ItemKind, opts?: MenuOptions | MenuItemOptions | SubmenuOptions | PredefinedMenuItemOptions | CheckMenuItemOptions | IconMenuItemOptions): Promise<[number, string]>;
export declare class MenuItemBase extends Resource {
    #private;
    /** The id of this item. */
    get id(): string;
    /** @ignore */
    get kind(): string;
    /** @ignore */
    protected constructor(rid: number, id: string, kind: ItemKind);
}
