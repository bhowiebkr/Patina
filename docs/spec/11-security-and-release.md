# Patina Specification: Security, Testing and Release

Part of the [Patina Specification](README.md).

## 11. Security

Build scripts and procedural macros run code at compile time. Native nodes run with full user rights.

- When the user opens a project from another person, Patina asks the user to trust it.
- An untrusted project opens read-only. The user can see the graph and the code. Patina does not build or run it.
- Until the user trusts a project, Patina starts no tool on it. The tools include the build, check and test commands, rust-analyzer, the formatter, views, and connection kinds. The **Code** tab shows the source with syntax colors only. rust-analyzer runs build scripts and procedural macros, so it starts only for a trusted project.
- Patina reads no tool settings from an untrusted project, for example `.cargo/config.toml` or `rust-toolchain.toml`.
- A folder that the user imports from an untrusted project stays out of the workspace members until the user trusts it (6.2).
- Patina keeps a list of trusted project folders. A project that the user makes in Patina goes on the list. The settings screen shows the list, and the user can remove a folder from it (15.10).

## 12. Testing

- Node authors can write `#[test]` functions in node code. Patina runs them with `cargo test`.
- Engine tests: slot and fuse, scheduler, clock domains, type keys, hot reload.
- CI: GitHub Actions builds and tests on Windows, Linux, and macOS for each release (13.5).

## 13. Distribution

### 13.1 Release

GitHub Releases has prebuilt files in two editions. Windows is the main platform. The Linux and macOS files are basic builds (13.5).

| Edition | Windows | macOS | Linux |
|---|---|---|---|
| Portable | `.zip` | None | `.zip` |
| Installed | `.msi` | `.dmg` | `.deb`, AppImage |

- **Portable:** the user unzips the file and starts Patina. The program, the toolchain, and the projects stay in the Patina folder. Patina makes no changes to the system and needs no admin rights.
- **Installed:** the user runs the installer of the platform. The program goes to the program folder of the platform. The toolchain and the projects go to the folders of the user (13.3).
- The `.msi` asks the user to install for **Just me** or for **All users**. **Just me** needs no admin rights. **All users** and the `.deb` package need admin rights.
- The portable zip holds a file named `portable` next to the executable. When this file is there, Patina keeps all its data in its own folder. When it is not there, Patina uses the folders of the user.
- macOS has no portable edition. macOS can start a downloaded app from a random, read-only copy (App Translocation). Thus an app cannot rely on files next to it. The only reliable fixes use private Apple functions or remove the download mark of macOS from the app. On macOS, Patina always uses the folders of the user, from any folder that the app is in.
- On macOS, the user drags Patina.app to **Applications**. Patina also runs from other folders.
- The macOS build has an ad-hoc signature and no notarization (13.5). macOS blocks it the first time. The user opens **System Settings > Privacy & Security** and clicks **Open Anyway**.
- No edition changes the `PATH` or installs Rust for the whole system.
- Patina is under the MIT license or the Apache License 2.0, as the user chooses (`MIT OR Apache-2.0`). The repository has the files `LICENSE-MIT` and `LICENSE-APACHE`.
- Each release file includes the license notices: the two Patina license files, and the license and `NOTICE` files of each crate in the build. The portable edition keeps them in a `licenses/` folder (13.3).
- `cargo run -p package` makes the portable zip (`tools/package`). It builds the page and the release programs `patina` and `patina-cli`. It copies the notice files of each crate and npm package in the build, without the dev-dependencies, and lists the license of each package in `licenses/README.txt`. The list marks a package that has no license file. On Windows, `tar.exe` of System32 writes the zip. On Linux, the `zip` program writes it. The **Package** workflow of GitHub Actions runs the tool on Windows and Linux when a person starts it or pushes a tag `v*`. The zips are artifacts of the run. This is an agent decision of 2026-11-15. The owner can change it.
- The installers do not include the toolchain. Both editions use the **Install toolchain** button (13.2).
- The `.msi` uses the Aged Dovetail artwork: `icons/wix-banner.bmp` at the top of its dialogs and `icons/wix-dialog.bmp` on its first and last pages. **Apps > Installed apps** shows the icon of the executable for the uninstall. This is an agent decision of 2026-11-15. The owner can change it.
- The `.msi` installs the WebView2 Runtime if it is missing, with the bootstrapper of Tauri. This needs internet.
- The portable edition checks for the WebView2 Runtime before it opens a window. If it is missing, Patina shows a Windows message with a link to the Microsoft download. The Runtime then installs for the current user.
- An error that stops Patina before or while its window opens shows in a message box of the platform, and goes to the standard error. The hidden self-check (`--check`) shows no message box. This is an agent decision of 2026-11-15. The owner can change it.

### 13.2 "Install toolchain" button

On first use, Patina installs these components into its toolchain folder (13.3):

1. **Rust:** `rustup-init` with `RUSTUP_HOME` and `CARGO_HOME` inside the toolchain folder, and `--no-modify-path`. Patina downloads `rustup-init`, the channel manifest and the packages of the minimal profile, and checks the SHA-256 hash of each file. Rustup then installs from this local copy of the server (`RUSTUP_DIST_SERVER` with a `file:` URL).
2. **C compiler and linker:** Many crates compile C code in build scripts. On Linux, rustc also needs `cc` as the linker driver. Preferred choice: **Zig** as a portable C compiler and linker for all three platforms (`zig cc`). On Windows, Zig compiles the C code, and `rust-lld` of the Rust toolchain links (see below). Fallback: the system compiler.
3. **rust-analyzer:** for autocomplete in the code editor. The install downloads its package with the packages of the minimal profile, and gives rustup `--component rust-analyzer`. The check runs `rust-analyzer --version`.
4. **NVRTC:** optional, for CUDA nodes.

- The toolchain alone builds nodes on a clean Windows computer, under a standard account: no Visual Studio, no Windows SDK, and no admin rights. Milestone 0 proves this (19).
- The Windows target is `x86_64-pc-windows-gnu`. Zig compiles the C code of crates. `rust-lld` links, with the start files and import libraries of the `rust-mingw` component. Patina does not use the GNU `ld` of `rust-mingw`: it cannot open a path with letters outside the ANSI code page, for example CJK letters. This is an agent decision of 2026-11-15, after the toolchain prototype ran on the computer of the owner (STATUS.md). The run on a clean computer is still open. The owner can change it.
- Patina starts the tools with a clean environment. `PATH` has only the Rust tools and System32, so a Visual Studio or a Rust install on the computer has no effect. Zig keeps its cache in the toolchain folder.
- Some firewalls allow only known programs. Patina makes all the downloads of the toolchain with the `curl.exe` of System32, so the user allows no new program. Cargo still downloads the crates. If its download fails, the toolchain panel names the `cargo.exe` of the toolchain folder. This is an agent decision of 2026-11-15.
- The fallback, the system compiler, is not portable. When Patina uses it, the toolchain panel says so.
- Crates with native dependencies, for example a C library that needs its own build system, work only when a test proves it.
- If an install step fails, for example with no internet, Patina shows the step and a **Retry** button. A component counts as installed only after Patina checks it.
- The install works in the folder `toolchain-install`. After the check, Patina renames it to `toolchain`, so Patina never uses a part install. A retry keeps the downloads with a correct hash. `tar.exe` of System32 unpacks Zig. The Zig wrappers are copies of the Patina program: a copy named `zig-cc.exe`, `zig-ar.exe` or `zig-dlltool.exe` starts Zig. rustc makes the import libraries of the shared `std` with `zig-dlltool.exe`, because the dlltool of binutils cannot open a path with CJK letters. In this version, the button works only on Windows. This is an agent decision of 2026-11-15. The owner can change it.
- Building Patina itself needs the Tauri prerequisites, for example Visual Studio Build Tools on Windows. That is a need of the developers of Patina, not of its users. The engine and the clusters can use different Windows targets, because they talk only through the C ABI (5.2).

### 13.3 Folder layout

Portable edition (Windows and Linux):

```
Patina/
  patina(.exe)
  patina-cli(.exe)
  portable         marks the portable edition (13.1)
  licenses/        the license notices (13.1)
  toolchain/
    rustup/        RUSTUP_HOME
    cargo/         CARGO_HOME + crate cache
    zig/           C compiler and its cache
    cuda/          NVRTC (optional)
  projects/
    <project>/     project folders (6.2)
  data/            settings, logs, the WebView2 data, and recovery files
```

- If the portable folder is read-only, Patina shows a message that names the folder, and stops. It does not write to the folders of the user instead.

Installed edition:

| Contents | Windows | macOS | Linux |
|---|---|---|---|
| Program | **Just me**: `%LOCALAPPDATA%\Programs\Patina`. **All users**: `%ProgramFiles%\Patina` | `/Applications/Patina.app` | `.deb`: `/usr/bin/patina`. AppImage: where the user puts it |
| Toolchain | `%LOCALAPPDATA%\Patina\toolchain` | `~/Library/Application Support/Patina/toolchain` | `~/.local/share/patina/toolchain` |
| Projects | `Documents\Patina` | `~/Documents/Patina` | `~/Documents/Patina` |
| Data: settings, logs, WebView2 data, recovery files | `%LOCALAPPDATA%\Patina\data` | `~/Library/Application Support/Patina/data` | `~/.local/share/patina/data` |

- The toolchain folder has the same contents in both editions.
- When the toolchain folder has no `cargo`, Patina uses the Rust of the system: the `CARGO` variable, or else `PATH`. Then a checkout of Patina builds projects without the folder. The settings screen shows the toolchain folder and the Rust that Patina uses. `patina-cli` uses the `toolchain/` folder next to it. This is an agent decision of 2026-11-15.
- The user can choose another folder for projects.
- Each user has their own toolchain and data folder. After an **All users** install, each user clicks **Install toolchain** one time.

**Uninstall:**

| Edition | Steps |
|---|---|
| Portable (Windows and Linux) | Delete the Patina folder. |
| Installed, Windows | Open **Settings > Apps > Installed apps**, and click **Uninstall** for Patina. |
| Installed, macOS | Move Patina.app to the Trash. |
| Installed, Linux | Run `sudo apt remove patina`, or delete the AppImage. |

- An uninstall of the installed edition removes only the program. The toolchain and the projects stay. To remove them, delete their folders.

### 13.4 Known system dependencies

| Platform | Dependency | Reason |
|---|---|---|
| Windows | WebView2 Runtime | Tauri draws the editor with it. Windows 11 includes it, and most Windows 10 computers get it with Microsoft Edge. It can still be missing, so Patina checks for it (13.1). |
| Windows | Code signing certificate (for the release) | Signing of the `.msi` and the executable. Without it, SmartScreen shows a warning. |
| Linux | WebKitGTK | Tauri uses the system web engine. Most desktop distributions include it. |
| macOS | Xcode Command Line Tools, possibly | Apple frameworks. Test if Zig removes this need. Patina starts the install with `xcode-select --install`. |
| macOS | Apple Developer account (optional, only for a notarized release) | Signing and notarization. The basic build has neither, so the user clicks **Open Anyway** one time (13.1). |
| macOS | `com.apple.security.cs.disable-library-validation` entitlement (only for a notarized release) | Notarization needs the hardened runtime. The hardened runtime loads only libraries that Apple or the same team signed. Patina loads clusters that it compiles on the computer of the user (8.5). The basic build has no hardened runtime. Each cluster still needs at least an ad-hoc signature on Apple silicon. |

### 13.5 Platform support

Windows is the main platform. Linux and macOS get basic support, and contributors who use them improve it.

| Platform | Support |
|---|---|
| Windows | Main. The owner builds, tests, and releases on Windows. Each milestone must work on Windows. |
| Linux | Basic. CI builds the release files and runs the tests. Contributors who use Linux test and improve them. |
| macOS | Basic. CI builds an ad-hoc signed `.dmg` and runs the tests. The build has no notarization (13.1). Contributors who use macOS test and improve it. |

- A gap on Linux or macOS does not block a milestone or a release. Write an issue for it, so that a contributor can take it.
- If the Linux or macOS build fails, the release leaves out the files of that platform. An issue tracks the failure.
- Keep the code cross-platform where Rust and Tauri allow it, so that the basic builds keep working.
- A contributor can raise the support of a platform, for example with a notarized macOS release (13.4). Ask the owner first.

## 14. Packaging

A package contains:

1. The engine runtime.
2. The baked graph.
3. The full project source: the project folders (6.2), without `build/`.
4. A header with the Patina version and the Rust version.
5. The license notices: the two Patina license files (13.1), and the license and `NOTICE` files of each crate from outside the project.

- The MIT license and the Apache License 2.0 require the notices in each copy. A package contains Patina code and the code of crates, so it includes the notices.
- Patina finds the notices in the Cargo metadata and the source folder of each crate in the build.
- If a crate from outside the project has no license and no license file, the package build shows a warning that names the crate. The build still makes the package.
- The project source is in the package already (item 3). The user chooses the license of the project.

| Package type | Use |
|---|---|
| Executable | Run from a command line. Graph inputs become arguments. |
| Library (C API) | Call from other software. |
| Python module (`pyo3`) | Call from Python. |

The editor can open a package and show the original project.
