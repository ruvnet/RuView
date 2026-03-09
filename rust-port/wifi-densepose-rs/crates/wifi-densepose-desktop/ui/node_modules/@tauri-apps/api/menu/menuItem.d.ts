import { MenuItemBase } from './base';
/** Options for creating a new menu item. */
export interface MenuItemOptions {
    /** Specify an id to use for the new menu item. */
    id?: string;
    /** The text of the new menu item. */
    text: string;
    /** Whether the new menu item is enabled or not. */
    enabled?: boolean;
    /** Specify an accelerator for the new menu item. */
    accelerator?: string;
    /** Specify a handler to be called when this menu item is activated. */
    action?: (id: string) => void;
}
/** A menu item inside a {@linkcode Menu} or {@linkcode Submenu} and contains only text. */
export declare class MenuItem extends MenuItemBase {
    /** @ignore */
    protected constructor(rid: number, id: string);
    /** Create a new menu item. */
    static new(opts: MenuItemOptions): Promise<MenuItem>;
    /** Returns the text of this menu item. */
    text(): Promise<string>;
    /** Sets the text for this menu item. */
    setText(text: string): Promise<void>;
    /** Returns whether this menu item is enabled or not. */
    isEnabled(): Promise<boolean>;
    /** Sets whether this menu item is enabled or not. */
    setEnabled(enabled: boolean): Promise<void>;
    /** Sets the accelerator for this menu item. */
    setAccelerator(accelerator: string | null): Promise<void>;
}
