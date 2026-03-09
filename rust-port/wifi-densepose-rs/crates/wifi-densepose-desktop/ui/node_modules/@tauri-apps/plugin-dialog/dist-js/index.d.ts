/**
 * Extension filters for the file dialog.
 *
 * @since 2.0.0
 */
interface DialogFilter {
    /** Filter name. */
    name: string;
    /**
     * Extensions to filter, without a `.` prefix.
     *
     * **Note:** Mobile platforms have different APIs for filtering that may not support extensions.
     * iOS: Extensions are supported in the document picker, but not in the media picker.
     * Android: Extensions are not supported.
     *
     * For these platforms, MIME types are the primary way to filter files, as opposed to extensions.
     * This means the string values here labeled as `extensions` may also be a MIME type.
     * This property name of `extensions` is being kept for backwards compatibility, but this may be revisited to
     * specify the difference between extension or MIME type filtering.
     *
     * @example
     * ```typescript
     * extensions: ['svg', 'png']
     * ```
     */
    extensions: string[];
}
/**
 * Options for the open dialog.
 *
 * @since 2.0.0
 */
interface OpenDialogOptions {
    /** The title of the dialog window (desktop only). */
    title?: string;
    /**
     * The filters of the dialog.
     * On mobile platforms, if either:
     * A) the {@linkcode pickerMode} is set to `media`, `image`, or `video`
     * -- or --
     * B) the filters include **only** either image or video mime types, the media picker will be displayed.
     * Otherwise, the document picker will be displayed.
     */
    filters?: DialogFilter[];
    /**
     * Initial directory or file path.
     * If it's a directory path, the dialog interface will change to that folder.
     * If it's not an existing directory, the file name will be set to the dialog's file name input and the dialog will be set to the parent folder.
     *
     * On mobile the file name is always used on the dialog's file name input.
     * If not provided, Android uses `(invalid).txt` as default file name.
     */
    defaultPath?: string;
    /** Whether the dialog allows multiple selection or not. */
    multiple?: boolean;
    /** Whether the dialog is a directory selection or not. */
    directory?: boolean;
    /**
     * If `directory` is true, indicates that it will be read recursively later.
     * Defines whether subdirectories will be allowed on the scope or not.
     */
    recursive?: boolean;
    /** Whether to allow creating directories in the dialog. Enabled by default. **macOS Only** */
    canCreateDirectories?: boolean;
    /**
     * The preferred mode of the dialog.
     * This is meant for mobile platforms (iOS and Android) which have distinct file and media pickers.
     * If not provided, the dialog will automatically choose the best mode based on the MIME types or extensions of the {@linkcode filters}.
     * On desktop, this option is ignored.
     */
    pickerMode?: PickerMode;
    /**
     * The file access mode of the dialog.
     * If not provided, `copy` is used, which matches the behavior of the {@linkcode open} method before the introduction of this option.
     *
     * **Usage**
     * If a file is opened with {@linkcode fileAccessMode: 'copy'}, it will be copied to the app's sandbox.
     * This means the file can be read, edited, deleted, copied, or any other operation without any issues, since the file
     * now belongs to the app.
     * This also means that the caller has responsibility of deleting the file if this file is not meant to be retained
     * in the app sandbox.
     *
     * If a file is opened with {@linkcode fileAccessMode: 'scoped'}, the file will remain in its original location
     * and security-scoped access will be automatically managed by the system.
     *
     * **Note**
     * This is specifically meant for document pickers on iOS or MacOS, in conjunction with [security scoped resources](https://developer.apple.com/documentation/foundation/nsurl/startaccessingsecurityscopedresource()).
     *
     * Why only document pickers, and not image or video pickers?
     * The image and video pickers on iOS behave differently from the document pickers, and return [NSItemProvider](https://developer.apple.com/documentation/foundation/nsitemprovider) objects instead of file URLs.
     * These are meant to be ephemeral (only available within the callback of the picker), and are not accessible outside of the callback.
     * So for image and video pickers, the only way to access the file is to copy it to the app's sandbox, and this is the URL that is returned from this API.
     * This means there is no provision for using `scoped` mode with image or video pickers.
     * If an image or video picker is used, `copy` is always used.
     */
    fileAccessMode?: FileAccessMode;
}
/**
 * Options for the save dialog.
 *
 * @since 2.0.0
 */
interface SaveDialogOptions {
    /** The title of the dialog window (desktop only). */
    title?: string;
    /** The filters of the dialog. */
    filters?: DialogFilter[];
    /**
     * Initial directory or file path.
     * If it's a directory path, the dialog interface will change to that folder.
     * If it's not an existing directory, the file name will be set to the dialog's file name input and the dialog will be set to the parent folder.
     *
     * On mobile the file name is always used on the dialog's file name input.
     * If not provided, Android uses `(invalid).txt` as default file name.
     */
    defaultPath?: string;
    /** Whether to allow creating directories in the dialog. Enabled by default. **macOS Only** */
    canCreateDirectories?: boolean;
}
/**
 * The preferred mode of the dialog.
 * This is meant for mobile platforms (iOS and Android) which have distinct file and media pickers.
 * On desktop, this option is ignored.
 * If not provided, the dialog will automatically choose the best mode based on the MIME types or extensions of the {@linkcode filters}.
 *
 * **Note:** This option is only supported on iOS 14 and above. This parameter is ignored on iOS 13 and below.
 */
export type PickerMode = 'document' | 'media' | 'image' | 'video';
/**
 * The file access mode of the dialog.
 *
 * - `copy`: copy/move the picked file to the app sandbox; no scoped access required.
 * - `scoped`: keep file in place; security-scoped access is automatically managed.
 *
 * **Note:** This option is only supported on iOS 14 and above. This parameter is ignored on iOS 13 and below.
 */
export type FileAccessMode = 'copy' | 'scoped';
/**
 * Default buttons for a message dialog.
 *
 * @since 2.4.0
 */
export type MessageDialogDefaultButtons = 'Ok' | 'OkCancel' | 'YesNo' | 'YesNoCancel';
/** All possible button keys. */
type ButtonKey = 'ok' | 'cancel' | 'yes' | 'no';
/** Ban everything except a set of keys. */
type BanExcept<Allowed extends ButtonKey> = Partial<Record<Exclude<ButtonKey, Allowed>, never>>;
/**
 * The Yes, No and Cancel buttons of a message dialog.
 *
 * @since 2.4.0
 */
export type MessageDialogButtonsYesNoCancel = {
    /** The Yes button. */
    yes: string;
    /** The No button. */
    no: string;
    /** The Cancel button. */
    cancel: string;
} & BanExcept<'yes' | 'no' | 'cancel'>;
/**
 * The Ok and Cancel buttons of a message dialog.
 *
 * @since 2.4.0
 */
export type MessageDialogButtonsOkCancel = {
    /** The Ok button. */
    ok: string;
    /** The Cancel button. */
    cancel: string;
} & BanExcept<'ok' | 'cancel'>;
/**
 * The Ok button of a message dialog.
 *
 * @since 2.4.0
 */
export type MessageDialogButtonsOk = {
    /** The Ok button. */
    ok: string;
} & BanExcept<'ok'>;
/**
 * Custom buttons for a message dialog.
 *
 * @since 2.4.0
 */
export type MessageDialogCustomButtons = MessageDialogButtonsYesNoCancel | MessageDialogButtonsOkCancel | MessageDialogButtonsOk;
/**
 * The buttons of a message dialog.
 *
 * @since 2.4.0
 */
export type MessageDialogButtons = MessageDialogDefaultButtons | MessageDialogCustomButtons;
/**
 * @since 2.0.0
 */
interface MessageDialogOptions {
    /** The title of the dialog. Defaults to the app name. */
    title?: string;
    /** The kind of the dialog. Defaults to `info`. */
    kind?: 'info' | 'warning' | 'error';
    /**
     * The label of the Ok button.
     *
     * @deprecated Use {@linkcode MessageDialogOptions.buttons} instead.
     */
    okLabel?: string;
    /**
     * The buttons of the dialog.
     *
     * @example
     *
     * ```ts
     * // Use system default buttons texts
     * await message('Hello World!', { buttons: 'Ok' })
     * await message('Hello World!', { buttons: 'OkCancel' })
     *
     * // Or with custom button texts
     * await message('Hello World!', { buttons: { ok: 'Yes!' } })
     * await message('Take on the task?', {
     *   buttons: { ok: 'Accept', cancel: 'Cancel' }
     * })
     * await message('Show the file content?', {
     *   buttons: { yes: 'Show content', no: 'Show in folder', cancel: 'Cancel' }
     * })
     * ```
     *
     * @since 2.4.0
     */
    buttons?: MessageDialogButtons;
}
interface ConfirmDialogOptions {
    /** The title of the dialog. Defaults to the app name. */
    title?: string;
    /** The kind of the dialog. Defaults to `info`. */
    kind?: 'info' | 'warning' | 'error';
    /** The label of the confirm button. */
    okLabel?: string;
    /** The label of the cancel button. */
    cancelLabel?: string;
}
type OpenDialogReturn<T extends OpenDialogOptions> = T['directory'] extends true ? T['multiple'] extends true ? string[] | null : string | null : T['multiple'] extends true ? string[] | null : string | null;
/**
 * Open a file/directory selection dialog.
 *
 * The selected paths are added to the filesystem and asset protocol scopes.
 * When security is more important than the easy of use of this API,
 * prefer writing a dedicated command instead.
 *
 * Note that the scope change is not persisted, so the values are cleared when the application is restarted.
 * You can save it to the filesystem using [tauri-plugin-persisted-scope](https://github.com/tauri-apps/tauri-plugin-persisted-scope).
 * @example
 * ```typescript
 * import { open } from '@tauri-apps/plugin-dialog';
 * // Open a selection dialog for image files
 * const selected = await open({
 *   multiple: true,
 *   filters: [{
 *     name: 'Image',
 *     extensions: ['png', 'jpeg']
 *   }]
 * });
 * if (Array.isArray(selected)) {
 *   // user selected multiple files
 * } else if (selected === null) {
 *   // user cancelled the selection
 * } else {
 *   // user selected a single file
 * }
 * ```
 *
 * @example
 * ```typescript
 * import { open } from '@tauri-apps/plugin-dialog';
 * import { appDir } from '@tauri-apps/api/path';
 * // Open a selection dialog for directories
 * const selected = await open({
 *   directory: true,
 *   multiple: true,
 *   defaultPath: await appDir(),
 * });
 * if (Array.isArray(selected)) {
 *   // user selected multiple directories
 * } else if (selected === null) {
 *   // user cancelled the selection
 * } else {
 *   // user selected a single directory
 * }
 * ```
 *
 * @returns A promise resolving to the selected path(s)
 *
 * @since 2.0.0
 */
declare function open<T extends OpenDialogOptions>(options?: T): Promise<OpenDialogReturn<T>>;
/**
 * Open a file/directory save dialog.
 *
 * The selected path is added to the filesystem and asset protocol scopes.
 * When security is more important than the easy of use of this API,
 * prefer writing a dedicated command instead.
 *
 * Note that the scope change is not persisted, so the values are cleared when the application is restarted.
 * You can save it to the filesystem using [tauri-plugin-persisted-scope](https://github.com/tauri-apps/tauri-plugin-persisted-scope).
 * @example
 * ```typescript
 * import { save } from '@tauri-apps/plugin-dialog';
 * const filePath = await save({
 *   filters: [{
 *     name: 'Image',
 *     extensions: ['png', 'jpeg']
 *   }]
 * });
 * ```
 *
 * @returns A promise resolving to the selected path.
 *
 * @since 2.0.0
 */
declare function save(options?: SaveDialogOptions): Promise<string | null>;
/**
 * The result of a message dialog.
 *
 * The result is a string if the dialog has custom buttons,
 * otherwise it is one of the default buttons.
 *
 * @since 2.4.0
 */
export type MessageDialogResult = 'Yes' | 'No' | 'Ok' | 'Cancel' | (string & {});
/**
 * Shows a message dialog with an `Ok` button.
 * @example
 * ```typescript
 * import { message } from '@tauri-apps/plugin-dialog';
 * await message('Tauri is awesome', 'Tauri');
 * await message('File not found', { title: 'Tauri', kind: 'error' });
 * ```
 *
 * @param message The message to show.
 * @param options The dialog's options. If a string, it represents the dialog title.
 *
 * @returns A promise indicating the success or failure of the operation.
 *
 * @since 2.0.0
 *
 */
declare function message(message: string, options?: string | MessageDialogOptions): Promise<MessageDialogResult>;
/**
 * Shows a question dialog with `Yes` and `No` buttons.
 * @example
 * ```typescript
 * import { ask } from '@tauri-apps/plugin-dialog';
 * const yes = await ask('Are you sure?', 'Tauri');
 * const yes2 = await ask('This action cannot be reverted. Are you sure?', { title: 'Tauri', kind: 'warning' });
 * ```
 *
 * @param message The message to show.
 * @param options The dialog's options. If a string, it represents the dialog title.
 *
 * @returns A promise resolving to a boolean indicating whether `Yes` was clicked or not.
 *
 * @since 2.0.0
 */
declare function ask(message: string, options?: string | ConfirmDialogOptions): Promise<boolean>;
/**
 * Shows a question dialog with `Ok` and `Cancel` buttons.
 * @example
 * ```typescript
 * import { confirm } from '@tauri-apps/plugin-dialog';
 * const confirmed = await confirm('Are you sure?', 'Tauri');
 * const confirmed2 = await confirm('This action cannot be reverted. Are you sure?', { title: 'Tauri', kind: 'warning' });
 * ```
 *
 * @param message The message to show.
 * @param options The dialog's options. If a string, it represents the dialog title.
 *
 * @returns A promise resolving to a boolean indicating whether `Ok` was clicked or not.
 *
 * @since 2.0.0
 */
declare function confirm(message: string, options?: string | ConfirmDialogOptions): Promise<boolean>;
export type { DialogFilter, OpenDialogOptions, OpenDialogReturn, SaveDialogOptions, MessageDialogOptions, ConfirmDialogOptions };
export { open, save, message, ask, confirm };
