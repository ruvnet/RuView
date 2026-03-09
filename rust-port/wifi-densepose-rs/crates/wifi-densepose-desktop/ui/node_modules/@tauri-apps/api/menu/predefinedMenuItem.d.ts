import { MenuItemBase } from './base';
import { Image } from '../image';
/** A metadata for the about predefined menu item. */
export interface AboutMetadata {
    /** Sets the application name. */
    name?: string;
    /** The application version. */
    version?: string;
    /**
     * The short version, e.g. "1.0".
     *
     * #### Platform-specific
     *
     * - **Windows / Linux:** Appended to the end of `version` in parentheses.
     */
    shortVersion?: string;
    /**
     * The authors of the application.
     *
     * #### Platform-specific
     *
     * - **macOS:** Unsupported.
     */
    authors?: string[];
    /**
     * Application comments.
     *
     * #### Platform-specific
     *
     * - **macOS:** Unsupported.
     */
    comments?: string;
    /** The copyright of the application. */
    copyright?: string;
    /**
     * The license of the application.
     *
     * #### Platform-specific
     *
     * - **macOS:** Unsupported.
     */
    license?: string;
    /**
     * The application website.
     *
     * #### Platform-specific
     *
     * - **macOS:** Unsupported.
     */
    website?: string;
    /**
     * The website label.
     *
     * #### Platform-specific
     *
     * - **macOS:** Unsupported.
     */
    websiteLabel?: string;
    /**
     * The credits.
     *
     * #### Platform-specific
     *
     * - **Windows / Linux:** Unsupported.
     */
    credits?: string;
    /**
     * The application icon.
     *
     * #### Platform-specific
     *
     * - **Windows:** Unsupported.
     */
    icon?: string | Uint8Array | ArrayBuffer | number[] | Image;
}
/** Options for creating a new predefined menu item. */
export interface PredefinedMenuItemOptions {
    /** The text of the new predefined menu item. */
    text?: string;
    /** The predefined item type */
    item: 'Separator' | 'Copy' | 'Cut' | 'Paste' | 'SelectAll' | 'Undo' | 'Redo' | 'Minimize' | 'Maximize' | 'Fullscreen' | 'Hide' | 'HideOthers' | 'ShowAll' | 'CloseWindow' | 'Quit' | 'Services' | {
        About: AboutMetadata | null;
    };
}
/** A predefined (native) menu item which has a predefined behavior by the OS or by tauri.  */
export declare class PredefinedMenuItem extends MenuItemBase {
    /** @ignore */
    protected constructor(rid: number, id: string);
    /** Create a new predefined menu item. */
    static new(opts?: PredefinedMenuItemOptions): Promise<PredefinedMenuItem>;
    /** Returns the text of this predefined menu item. */
    text(): Promise<string>;
    /** Sets the text for this predefined menu item. */
    setText(text: string): Promise<void>;
}
