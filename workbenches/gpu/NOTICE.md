# Material sources

The material workbench uses Millwright Surface source revision
`2f47ff8160c2bd8362dee42b6bd4f11e460e06c8`.

The `types/forge-*` crates and the `nodes/material-*/assets/kernel.wesl` files use MPL-2.0.
Each copied crate contains the original LICENSE file.
[UPSTREAM.txt](types/forge-runtime/UPSTREAM.txt) records the original source hashes.
Cargo paths and asset paths changed for Patina's project layout.
Four source files had text punctuation converted to ASCII.
The native executor and GPU error handling are local adaptations.
The ZIP writer comes from Surface's `forge-ui/src/export_archive.rs` under MPL-2.0.
The PBR pass converter follows `forge-ui/src/export.rs` and adds a separate opacity pass.
The object adapter and graph target transport are local workbench code.

The copper recipe retains its original PNG bytes, sources, shaders, and resource metadata.
The preview mesh, baked maps, and studio HDR come from the same source revision.
The HDR resource has no separate source or license field in the recipe.
Its redistribution terms remain unverified. This example adds no license claim for that resource.

The Windows x64 GPU dependency links DXC through `wgpu` and `mach-dxcompiler-rs`.
Cargo.lock pins that dependency. Its build checks the downloaded compiler archive's SHA256.
