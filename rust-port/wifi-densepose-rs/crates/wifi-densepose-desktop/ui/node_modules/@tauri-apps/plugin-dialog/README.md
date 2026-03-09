![plugin-dialog](https://github.com/tauri-apps/plugins-workspace/raw/v2/plugins/dialog/banner.png)

Native system dialogs for opening and saving files along with message dialogs.

| Platform | Supported |
| -------- | --------- |
| Linux    | ✓         |
| Windows  | ✓         |
| macOS    | ✓         |
| Android  | ✓         |
| iOS      | ✓         |

## Install

_This plugin requires a Rust version of at least **1.77.2**_

There are three general methods of installation that we can recommend.

1. Use crates.io and npm (easiest, and requires you to trust that our publishing pipeline worked)
2. Pull sources directly from Github using git tags / revision hashes (most secure)
3. Git submodule install this repo in your tauri project and then use file protocol to ingest the source (most secure, but inconvenient to use)

Install the Core plugin by adding the following to your `Cargo.toml` file:

`src-tauri/Cargo.toml`

```toml
[dependencies]
tauri-plugin-dialog = "2.0.0"
# alternatively with Git:
tauri-plugin-dialog = { git = "https://github.com/tauri-apps/plugins-workspace", branch = "v2" }
```

### Linux XDG Desktop Portal Support

By default, this plugin uses gtk to show dialogs, however since `v2.5.0` you can switch to using [XDG Desktop Portal](https://flatpak.github.io/xdg-desktop-portal/) by adding the following to your `Cargo.toml` file:

```toml
[dependencies]
tauri-plugin-dialog = { version = "2.5.0", default-features = false, features = ["xdg-portal"] }
# alternatively with Git:
tauri-plugin-dialog = { git = "https://github.com/tauri-apps/plugins-workspace", branch = "v2", default-features = false, features = ["xdg-portal"] }

```

Do note if you use the `xdg-portal` feature, you need to ensure that [`zenity`](https://gitlab.gnome.org/GNOME/zenity) and an [XDG Desktop Portal backend](https://flatpak.github.io/xdg-desktop-portal#using-portals) is installed with your program.

For more information, see [XDG Desktop Portal documentation](https://flatpak.github.io/xdg-desktop-portal/) and [`rfd` documentation](https://docs.rs/rfd/latest/rfd#xdg-desktop-portal-backend).

### JavaScript

You can install the JavaScript Guest bindings using your preferred JavaScript package manager:

```sh
pnpm add @tauri-apps/plugin-dialog
# or
npm add @tauri-apps/plugin-dialog
# or
yarn add @tauri-apps/plugin-dialog
```

## Usage

First you need to register the core plugin with Tauri:

`src-tauri/src/lib.rs`

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Afterwards all the plugin's APIs are available through the JavaScript guest bindings:

```javascript

```

## Contributing

PRs accepted. Please make sure to read the Contributing Guide before making a pull request.

## Partners

<table>
  <tbody>
    <tr>
      <td align="center" valign="middle">
        <a href="https://crabnebula.dev" target="_blank">
          <img src="https://github.com/tauri-apps/plugins-workspace/raw/v2/.github/sponsors/crabnebula.svg" alt="CrabNebula" width="283">
        </a>
      </td>
    </tr>
  </tbody>
</table>

For the complete list of sponsors please visit our [website](https://tauri.app#sponsors) and [Open Collective](https://opencollective.com/tauri).

## License

Code: (c) 2015 - Present - The Tauri Programme within The Commons Conservancy.

MIT or MIT/Apache 2.0 where applicable.
