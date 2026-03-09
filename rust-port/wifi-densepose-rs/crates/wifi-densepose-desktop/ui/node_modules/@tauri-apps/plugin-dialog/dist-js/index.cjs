'use strict';

var core = require('@tauri-apps/api/core');

// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
/**
 * Internal function to convert the buttons to the Rust type.
 */
function buttonsToRust(buttons) {
    if (buttons === undefined) {
        return undefined;
    }
    if (typeof buttons === 'string') {
        return buttons;
    }
    else if ('ok' in buttons && 'cancel' in buttons) {
        return { OkCancelCustom: [buttons.ok, buttons.cancel] };
    }
    else if ('yes' in buttons && 'no' in buttons && 'cancel' in buttons) {
        return {
            YesNoCancelCustom: [buttons.yes, buttons.no, buttons.cancel]
        };
    }
    else if ('ok' in buttons) {
        return { OkCustom: buttons.ok };
    }
    return undefined;
}
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
async function open(options = {}) {
    if (typeof options === 'object') {
        Object.freeze(options);
    }
    return await core.invoke('plugin:dialog|open', { options });
}
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
async function save(options = {}) {
    if (typeof options === 'object') {
        Object.freeze(options);
    }
    return await core.invoke('plugin:dialog|save', { options });
}
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
async function message(message, options) {
    const opts = typeof options === 'string' ? { title: options } : options;
    return core.invoke('plugin:dialog|message', {
        message: message.toString(),
        title: opts?.title?.toString(),
        kind: opts?.kind,
        okButtonLabel: opts?.okLabel?.toString(),
        buttons: buttonsToRust(opts?.buttons)
    });
}
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
async function ask(message, options) {
    const opts = typeof options === 'string' ? { title: options } : options;
    return await core.invoke('plugin:dialog|ask', {
        message: message.toString(),
        title: opts?.title?.toString(),
        kind: opts?.kind,
        yesButtonLabel: opts?.okLabel?.toString(),
        noButtonLabel: opts?.cancelLabel?.toString()
    });
}
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
async function confirm(message, options) {
    const opts = typeof options === 'string' ? { title: options } : options;
    return await core.invoke('plugin:dialog|confirm', {
        message: message.toString(),
        title: opts?.title?.toString(),
        kind: opts?.kind,
        okButtonLabel: opts?.okLabel?.toString(),
        cancelButtonLabel: opts?.cancelLabel?.toString()
    });
}

exports.ask = ask;
exports.confirm = confirm;
exports.message = message;
exports.open = open;
exports.save = save;
