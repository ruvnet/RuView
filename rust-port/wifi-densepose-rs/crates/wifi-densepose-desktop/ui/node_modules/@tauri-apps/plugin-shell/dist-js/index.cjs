'use strict';

var core = require('@tauri-apps/api/core');

// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
/**
 * Access the system shell.
 * Allows you to spawn child processes and manage files and URLs using their default application.
 *
 * ## Security
 *
 * This API has a scope configuration that forces you to restrict the programs and arguments that can be used.
 *
 * ### Restricting access to the {@link open | `open`} API
 *
 * On the configuration object, `open: true` means that the {@link open} API can be used with any URL,
 * as the argument is validated with the `^((mailto:\w+)|(tel:\w+)|(https?://\w+)).+` regex.
 * You can change that regex by changing the boolean value to a string, e.g. `open: ^https://github.com/`.
 *
 * ### Restricting access to the {@link Command | `Command`} APIs
 *
 * The plugin permissions object has a `scope` field that defines an array of CLIs that can be used.
 * Each CLI is a configuration object `{ name: string, cmd: string, sidecar?: bool, args?: boolean | Arg[] }`.
 *
 * - `name`: the unique identifier of the command, passed to the {@link Command.create | Command.create function}.
 * If it's a sidecar, this must be the value defined on `tauri.conf.json > bundle > externalBin`.
 * - `cmd`: the program that is executed on this configuration. If it's a sidecar, this value is ignored.
 * - `sidecar`: whether the object configures a sidecar or a system program.
 * - `args`: the arguments that can be passed to the program. By default no arguments are allowed.
 *   - `true` means that any argument list is allowed.
 *   - `false` means that no arguments are allowed.
 *   - otherwise an array can be configured. Each item is either a string representing the fixed argument value
 *     or a `{ validator: string }` that defines a regex validating the argument value.
 *
 * #### Example scope configuration
 *
 * CLI: `git commit -m "the commit message"`
 *
 * Capability:
 * ```json
 * {
 *   "permissions": [
 *     {
 *       "identifier": "shell:allow-execute",
 *       "allow": [
 *         {
 *           "name": "run-git-commit",
 *           "cmd": "git",
 *           "args": ["commit", "-m", { "validator": "\\S+" }]
 *         }
 *       ]
 *     }
 *   ]
 * }
 * ```
 * Usage:
 * ```typescript
 * import { Command } from '@tauri-apps/plugin-shell'
 * Command.create('run-git-commit', ['commit', '-m', 'the commit message'])
 * ```
 *
 * Trying to execute any API with a program not configured on the scope results in a promise rejection due to denied access.
 *
 * @module
 */
/**
 * @since 2.0.0
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
class EventEmitter {
    constructor() {
        /** @ignore */
        // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment, @typescript-eslint/no-explicit-any
        this.eventListeners = Object.create(null);
    }
    /**
     * Alias for `emitter.on(eventName, listener)`.
     *
     * @since 2.0.0
     */
    addListener(eventName, listener) {
        return this.on(eventName, listener);
    }
    /**
     * Alias for `emitter.off(eventName, listener)`.
     *
     * @since 2.0.0
     */
    removeListener(eventName, listener) {
        return this.off(eventName, listener);
    }
    /**
     * Adds the `listener` function to the end of the listeners array for the
     * event named `eventName`. No checks are made to see if the `listener` has
     * already been added. Multiple calls passing the same combination of `eventName`and `listener` will result in the `listener` being added, and called, multiple
     * times.
     *
     * Returns a reference to the `EventEmitter`, so that calls can be chained.
     *
     * @since 2.0.0
     */
    on(eventName, listener) {
        if (eventName in this.eventListeners) {
            // eslint-disable-next-line security/detect-object-injection
            this.eventListeners[eventName].push(listener);
        }
        else {
            // eslint-disable-next-line security/detect-object-injection
            this.eventListeners[eventName] = [listener];
        }
        return this;
    }
    /**
     * Adds a **one-time**`listener` function for the event named `eventName`. The
     * next time `eventName` is triggered, this listener is removed and then invoked.
     *
     * Returns a reference to the `EventEmitter`, so that calls can be chained.
     *
     * @since 2.0.0
     */
    once(eventName, listener) {
        const wrapper = (arg) => {
            this.removeListener(eventName, wrapper);
            listener(arg);
        };
        return this.addListener(eventName, wrapper);
    }
    /**
     * Removes the all specified listener from the listener array for the event eventName
     * Returns a reference to the `EventEmitter`, so that calls can be chained.
     *
     * @since 2.0.0
     */
    off(eventName, listener) {
        if (eventName in this.eventListeners) {
            // eslint-disable-next-line security/detect-object-injection
            this.eventListeners[eventName] = this.eventListeners[eventName].filter((l) => l !== listener);
        }
        return this;
    }
    /**
     * Removes all listeners, or those of the specified eventName.
     *
     * Returns a reference to the `EventEmitter`, so that calls can be chained.
     *
     * @since 2.0.0
     */
    removeAllListeners(event) {
        if (event) {
            // eslint-disable-next-line security/detect-object-injection
            delete this.eventListeners[event];
        }
        else {
            // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment
            this.eventListeners = Object.create(null);
        }
        return this;
    }
    /**
     * @ignore
     * Synchronously calls each of the listeners registered for the event named`eventName`, in the order they were registered, passing the supplied arguments
     * to each.
     *
     * @returns `true` if the event had listeners, `false` otherwise.
     *
     * @since 2.0.0
     */
    emit(eventName, arg) {
        if (eventName in this.eventListeners) {
            // eslint-disable-next-line security/detect-object-injection
            const listeners = this.eventListeners[eventName];
            for (const listener of listeners)
                listener(arg);
            return true;
        }
        return false;
    }
    /**
     * Returns the number of listeners listening to the event named `eventName`.
     *
     * @since 2.0.0
     */
    listenerCount(eventName) {
        if (eventName in this.eventListeners)
            // eslint-disable-next-line security/detect-object-injection
            return this.eventListeners[eventName].length;
        return 0;
    }
    /**
     * Adds the `listener` function to the _beginning_ of the listeners array for the
     * event named `eventName`. No checks are made to see if the `listener` has
     * already been added. Multiple calls passing the same combination of `eventName`and `listener` will result in the `listener` being added, and called, multiple
     * times.
     *
     * Returns a reference to the `EventEmitter`, so that calls can be chained.
     *
     * @since 2.0.0
     */
    prependListener(eventName, listener) {
        if (eventName in this.eventListeners) {
            // eslint-disable-next-line security/detect-object-injection
            this.eventListeners[eventName].unshift(listener);
        }
        else {
            // eslint-disable-next-line security/detect-object-injection
            this.eventListeners[eventName] = [listener];
        }
        return this;
    }
    /**
     * Adds a **one-time**`listener` function for the event named `eventName` to the_beginning_ of the listeners array. The next time `eventName` is triggered, this
     * listener is removed, and then invoked.
     *
     * Returns a reference to the `EventEmitter`, so that calls can be chained.
     *
     * @since 2.0.0
     */
    prependOnceListener(eventName, listener) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const wrapper = (arg) => {
            this.removeListener(eventName, wrapper);
            // eslint-disable-next-line @typescript-eslint/no-unsafe-argument
            listener(arg);
        };
        return this.prependListener(eventName, wrapper);
    }
}
/**
 * @since 2.0.0
 */
class Child {
    constructor(pid) {
        this.pid = pid;
    }
    /**
     * Writes `data` to the `stdin`.
     *
     * @param data The message to write, either a string or a byte array.
     * @example
     * ```typescript
     * import { Command } from '@tauri-apps/plugin-shell';
     * const command = Command.create('node');
     * const child = await command.spawn();
     * await child.write('message');
     * await child.write([0, 1, 2, 3, 4, 5]);
     * ```
     *
     * @returns A promise indicating the success or failure of the operation.
     *
     * @since 2.0.0
     */
    async write(data) {
        await core.invoke('plugin:shell|stdin_write', {
            pid: this.pid,
            buffer: data
        });
    }
    /**
     * Kills the child process.
     *
     * @returns A promise indicating the success or failure of the operation.
     *
     * @since 2.0.0
     */
    async kill() {
        await core.invoke('plugin:shell|kill', {
            cmd: 'killChild',
            pid: this.pid
        });
    }
}
/**
 * The entry point for spawning child processes.
 * It emits the `close` and `error` events.
 * @example
 * ```typescript
 * import { Command } from '@tauri-apps/plugin-shell';
 * const command = Command.create('node');
 * command.on('close', data => {
 *   console.log(`command finished with code ${data.code} and signal ${data.signal}`)
 * });
 * command.on('error', error => console.error(`command error: "${error}"`));
 * command.stdout.on('data', line => console.log(`command stdout: "${line}"`));
 * command.stderr.on('data', line => console.log(`command stderr: "${line}"`));
 *
 * const child = await command.spawn();
 * console.log('pid:', child.pid);
 * ```
 *
 * @since 2.0.0
 *
 */
class Command extends EventEmitter {
    /**
     * @ignore
     * Creates a new `Command` instance.
     *
     * @param program The program name to execute.
     * It must be configured in your project's capabilities.
     * @param args Program arguments.
     * @param options Spawn options.
     */
    constructor(program, args = [], options) {
        super();
        /** Event emitter for the `stdout`. Emits the `data` event. */
        this.stdout = new EventEmitter();
        /** Event emitter for the `stderr`. Emits the `data` event. */
        this.stderr = new EventEmitter();
        this.program = program;
        this.args = typeof args === 'string' ? [args] : args;
        this.options = options ?? {};
    }
    /**
     * Creates a command to execute the given program.
     * @example
     * ```typescript
     * import { Command } from '@tauri-apps/plugin-shell';
     * const command = Command.create('my-app', ['run', 'tauri']);
     * const output = await command.execute();
     * ```
     *
     * @param program The program to execute.
     * It must be configured in your project's capabilities.
     */
    static create(program, args = [], options) {
        return new Command(program, args, options);
    }
    /**
     * Creates a command to execute the given sidecar program.
     * @example
     * ```typescript
     * import { Command } from '@tauri-apps/plugin-shell';
     * const command = Command.sidecar('my-sidecar');
     * const output = await command.execute();
     * ```
     *
     * @param program The program to execute.
     * It must be configured in your project's capabilities.
     */
    static sidecar(program, args = [], options) {
        const instance = new Command(program, args, options);
        instance.options.sidecar = true;
        return instance;
    }
    /**
     * Executes the command as a child process, returning a handle to it.
     *
     * @returns A promise resolving to the child process handle.
     *
     * @since 2.0.0
     */
    async spawn() {
        const program = this.program;
        const args = this.args;
        const options = this.options;
        if (typeof args === 'object') {
            Object.freeze(args);
        }
        const onEvent = new core.Channel();
        onEvent.onmessage = (event) => {
            switch (event.event) {
                case 'Error':
                    this.emit('error', event.payload);
                    break;
                case 'Terminated':
                    this.emit('close', event.payload);
                    break;
                case 'Stdout':
                    this.stdout.emit('data', event.payload);
                    break;
                case 'Stderr':
                    this.stderr.emit('data', event.payload);
                    break;
            }
        };
        return await core.invoke('plugin:shell|spawn', {
            program,
            args,
            options,
            onEvent
        }).then((pid) => new Child(pid));
    }
    /**
     * Executes the command as a child process, waiting for it to finish and collecting all of its output.
     * @example
     * ```typescript
     * import { Command } from '@tauri-apps/plugin-shell';
     * const output = await Command.create('echo', 'message').execute();
     * assert(output.code === 0);
     * assert(output.signal === null);
     * assert(output.stdout === 'message');
     * assert(output.stderr === '');
     * ```
     *
     * @returns A promise resolving to the child process output.
     *
     * @since 2.0.0
     */
    async execute() {
        const program = this.program;
        const args = this.args;
        const options = this.options;
        if (typeof args === 'object') {
            Object.freeze(args);
        }
        return await core.invoke('plugin:shell|execute', {
            program,
            args,
            options
        });
    }
}
/**
 * Opens a path or URL with the system's default app,
 * or the one specified with `openWith`.
 *
 * The `openWith` value must be one of `firefox`, `google chrome`, `chromium` `safari`,
 * `open`, `start`, `xdg-open`, `gio`, `gnome-open`, `kde-open` or `wslview`.
 *
 * @example
 * ```typescript
 * import { open } from '@tauri-apps/plugin-shell';
 * // opens the given URL on the default browser:
 * await open('https://github.com/tauri-apps/tauri');
 * // opens the given URL using `firefox`:
 * await open('https://github.com/tauri-apps/tauri', 'firefox');
 * // opens a file using the default program:
 * await open('/path/to/file');
 * ```
 *
 * @param path The path or URL to open.
 * This value is matched against the string regex defined on `tauri.conf.json > plugins > shell > open`,
 * which defaults to `^((mailto:\w+)|(tel:\w+)|(https?://\w+)).+`.
 * @param openWith The app to open the file or URL with.
 * Defaults to the system default application for the specified path type.
 *
 * @since 2.0.0
 */
async function open(path, openWith) {
    await core.invoke('plugin:shell|open', {
        path,
        with: openWith
    });
}

exports.Child = Child;
exports.Command = Command;
exports.EventEmitter = EventEmitter;
exports.open = open;
