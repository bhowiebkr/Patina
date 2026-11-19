# Vendored C++ sources

`build.rs` compiles these sources with the `cc` crate, so a build needs no CMake and no download.

| Library | Version | License | Files |
|---|---|---|---|
| [Manifold](https://github.com/elalish/manifold) | v3.5.5, commit 32271a1a | Apache-2.0, `manifold/LICENSE` | `include`, the sources of `src` that the build needs, the C bindings |
| [Clipper2](https://github.com/AngusJohnson/Clipper2) | 2.0.1, commit 46f63917 | BSL-1.0, `clipper2/LICENSE` | `CPP/Clipper2Lib/include` and `src` |

- The files are not changed. Some files have their own license notice at the top. Keep it.
- Left out: the tests, the samples, the other bindings, the CMake files and the `boolean2`
  backend of the cross sections.
- To update a library, copy the same files from the new version. Then change this table and
  the file list in `build.rs`.
