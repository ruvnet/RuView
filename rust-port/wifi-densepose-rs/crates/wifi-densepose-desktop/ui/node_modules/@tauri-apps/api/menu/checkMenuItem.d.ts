import { MenuItemBase } from './base';
import { type MenuItemOptions } from '../menu';
/** Options for creating a new check menu item. */
export interface CheckMenuItemOptions extends MenuItemOptions {
    /** Whether the new check menu item is enabled or not. */
    checked?: boolean;
}
/**
 * A check menu item inside a {@linkcode Menu} or {@linkcode Submenu}
 * and usually contains a text and a check mark or a similar toggle
 * that corresponds to a checked and unchecked states.
 */
export declare class CheckMenuItem extends MenuItemBase {
    /** @ignore */
    protected constructor(rid: number, id: string);
    /** Create a new check menu item. */
    static new(opts: CheckMenuItemOptions): Promise<CheckMenuItem>;
    /** Returns the text of this check menu item. */
    text(): Promise<string>;
    /** Sets the text for this check menu item. */
    setText(text: string): Promise<void>;
    /** Returns whether this check menu item is enabled or not. */
    isEnabled(): Promise<boolean>;
    /** Sets whether this check menu item is enabled or not. */
    setEnabled(enabled: boolean): Promise<void>;
    /** Sets the accelerator for this check menu item. */
    setAccelerator(accelerator: string | null): Promise<void>;
    /** Returns whether this check menu item is checked or not. */
    isChecked(): Promise<boolean>;
    /** Sets whether this check menu item is checked or not. */
    setChecked(checked: boolean): Promise<void>;
}
