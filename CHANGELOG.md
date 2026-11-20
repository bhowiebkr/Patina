# Changelog

This file lists the changes of each Patina release. The newest release is at the top.

Patina uses [semantic versioning](https://semver.org/). Before version 1.0, these rules apply:

- A new minor version, for example 0.2.0, can change the project format, the node API and the
  C API.
- A new patch version, for example 0.1.1, only fixes bugs. Projects of 0.1.0 open with no
  change.

## 0.1.0 - 2026-11-20

The first release. Windows is the main platform. The Linux build is basic.

### Downloads

| File | Edition |
|---|---|
| `patina-0.1.0-windows-x86_64.zip` | The portable edition for Windows. |
| `patina-0.1.0-linux-x86_64.zip` | The portable edition for Linux, a basic build. |

Unzip the file and start `patina`. Then open **Settings** and click **Install toolchain** one
time. The [README](https://github.com/bhowiebkr/Patina#install) has the full steps.

### What this release has

- **The editor.** A canvas for node graphs, node GUIs, wires that show their live values, the
  **Inspector**, docks for custom views, a code editor with completion, and undo.
- **The engine.** Patina compiles a graph to native code and runs the independent nodes in
  parallel. A change to one node compiles only that node and swaps it in while the graph runs.
- **Clock domains.** Batch, frame, audio and event domains.
- **Logic circuits.** A graph of NAND gates is a digital circuit, with one clock step in each
  frame. The computer workbench has a CPU made of gates.
- **GPU.** WESL shaders through `wgpu`, and CUDA through NVRTC on NVIDIA.
- **The command line.** `patina-cli` runs a project, renders its audio to a file and makes a
  package, with no window.
- **Packages.** A graph becomes an executable or a library with a C API.
- **Shared libraries.** Add a project from a local folder or from a Git tag. The index in
  `libraries.toml` lists the shared libraries.
- **The toolchain button.** **Install toolchain** downloads Rust and Zig into the Patina folder,
  about 265 MB. It changes no system settings.
- **Eight workbenches.** hello, computer, gpu, synth, model, city, gun and laser-level. The
  [workbench guide](https://github.com/bhowiebkr/Patina/blob/main/docs/workbenches/README.md) explains each workbench with images.
- **Check for updates.** The **About** dialog compares the version with the newest release on
  GitHub.

### Known limits

- The installers (`.msi`, `.dmg`, `.deb` and AppImage) come in a later release.
- The program has no code signature. Windows SmartScreen can show a warning the first time.
- **Install toolchain** works only on Windows. On Linux, Patina uses the Rust of the system.
- This release has no file for macOS.
- The Python module of a package comes in a later release.

[STATUS.md](https://github.com/bhowiebkr/Patina/blob/main/STATUS.md) has the full list of what works and the known limits.
