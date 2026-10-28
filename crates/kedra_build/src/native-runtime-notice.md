# AppImage native runtime and relink materials

The outer AppImage runtime is separate from the Rust application. It contains
libfuse under GNU LGPL 2.1, musl (MIT and notices in COPYRIGHT), squashfuse (BSD),
zstd (BSD), zlib (Zlib), mimalloc (MIT), and GCC startup/runtime code (GPL-3.0
with the GCC Runtime Library Exception). Their complete notice texts accompany
this file. The original source archives retain all file-level copyrights.
AppRun is MIT: its complete source and notice are in `AppRun.c`.

You may modify this runtime for your own use, replace/relink libfuse, and reverse
engineer the work for debugging those modifications. Nothing in Kedra's terms
restricts those LGPL rights. See `fuse-3.15.0.tar.xz-1.NOTICE` for the full LGPL.

## Exact provenance

Runtime source: AppImage/type2-runtime
`fedfad51acc2860b2916a1eef32ba4a500ff31ef`. Packaging uses the official x86-64
runtime binary, SHA-256
`617aa77314d57c8b8e4fae9c3f0b35af7b30d5529cb69e98f64f92510ed98a46`.
Its executable bytes form the distributed AppImage's prefix; a second executable
copy is not included in these materials. appimagetool fills
the standard 16-byte `.digest_md5` section; every other prefix byte is verified
against this input. ELF section boundaries come from the pinned input, not from
the generated image.

Official build:
<https://github.com/AppImage/type2-runtime/actions/runs/39320578745>.
It identifies libfuse 3.15.0, squashfuse 0.5.2 and Alpine 3.21 x86-64 packages
musl 1.2.5-r11, zstd 1.5.6-r2, zlib 1.3.2-r1 and mimalloc2 2.1.7-r0.
`materials.json` binds each bundled input to its upstream URL and SHA-256.

libfuse is modified by the AppImage project: `mount.c.diff`, dated 2024-11-24,
changes fusermount discovery to the `FUSERMOUNT_PROG` environment variable.
The exact patch is included in the runtime source archive under
`patches/libfuse/mount.c.diff`; upstream commit
`f5636118e0d8645e7f572771b5ac98b6da931f95` records it. No other libfuse source
modifications are used in the official binary.

The accompanying Alpine APKs provide static objects, CRT objects and headers
for relinking, including Alpine's musl/zlib fixes. The musl and musl-dev inputs
are pinned to 1.2.5-r12, not the official binary's 1.2.5-r11. This is an updated
replacement libc for recipient rebuilds, not the original build's exact objects.
Alpine [commit 5245849][musl-r12]
adds the CVE-2026-107891 regcomp fix in r12; it does not patch the prebuilt runtime.
The other APK versions match the recorded official build. The upstream musl/zlib
archives are supplemental upstream source, **not**
a claim to contain Alpine's downstream patches. These permissive libraries are
provided as linkable object code; libfuse and the complete runtime/squashfuse
sources and build scripts are provided to allow replacement and relinking.

The original build also uses standard GCC 14.2.0-r4 startup/runtime objects through
Alpine clang 19.1.4-r0, including crtbegin/crtend (`crtstuff.c`). The full GPLv3
license and GCC Runtime Library Exception 3.1 are in `GCC-COPYING3` and
`GCC-COPYING.RUNTIME`; the corresponding upstream startup source is `GCC-crtstuff.c`.
These standard compiler components are **not** supplied as binary objects in this
kit. Relinking acquires them through the normal Alpine toolchain. This relies on
LGPL 2.1 section 6's standard-toolchain exception and GCC's runtime exception,
not a claim that every implicitly linked object is included here. The documented
build invokes ordinary clang/GCC tooling without non-eligible compiler plugins.
The rebuild records its actual compiler/package versions in `toolchain-packages.txt`
and its actual linked input map in `runtime-link.map`; CI retains both as evidence.
A recipient rebuilding later may obtain different standard-toolchain versions.

The AppRun mirror is
<https://github.com/tauri-apps/binary-releases/releases/tag/apprun-old>.
The included helper is checksum-verified as
`f30140a43a0a59e46db21bdefdf749b9e9f2c6946e92afabbacf98b8ae73fb4f`.
The MIT copyright notice is preserved from AppImageKit's `src/AppRun.c`, pinned
to `2b8f5b39c41d3895e6b6b88a45b1905f4ffce4ef`. No claim of bit-identical
recompilation of that older MIT-only mirrored helper is made.

The mirrored AppRun identifies GCC 4.8.5 (Red Hat 4.8.5-44) startup objects and
GCC 9.3.1 (Red Hat 9.3.1-2) compilation in its ELF `.comment`. These also use the
GCC Runtime Library Exception 3.1; the license/exception texts accompany this kit.
Its only dynamic dependency is the host's `libc.so.6`; that glibc is not bundled
inside AppRun. The supplied GCC 14.2 startup source is provenance for the outer
runtime, not a claim of corresponding source for the older helper's startup code.

## Replace libfuse and relink

Use a native x86-64 Linux Docker host with internet for standard compiler/build
tools. No application source checkout is required. In this directory:

```sh
mkdir work
sh rebuild.sh prepare "$PWD/work"
# Modify the LGPL source under work/fuse, if desired.
sh rebuild.sh build "$PWD/work"
./work/runtime-x86_64 --appimage-version
```

The build uses the included sources, AppImage patch, upstream Makefile and
checksum-pinned relink objects, including the replacement musl revision above.
The compiler/build utilities are standard Alpine tools;
the process is not claimed hermetic or byte-for-byte reproducible. The output is
an unsigned, replaceable runtime, not a signed update or a trusted publisher.
To create an AppImage using it, use appimagetool's standard `--runtime-file`
option (or linuxdeploy-plugin-appimage's `LDAI_RUNTIME_FILE`) with an AppDir.
Keep the original image and work in a new output directory.

If an upstream moving download changes, Kedra packaging fails its checksum rather
than silently embedding an uncovered runtime. The bundled materials remain
available for recipients independently of those moving download locations.

[musl-r12]: https://github.com/alpinelinux/aports/commit/5245849e14d213abe18d589b4b80d65793327940
