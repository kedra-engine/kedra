#!/bin/sh
# Kedra: MIT OR Apache-2.0. Upstream build systems own compilation/linking.
set -eu

mode=${1:?Usage: sh rebuild.sh prepare|build ABSOLUTE_WORK_DIRECTORY}
work=${2:?An absolute work directory is required}
materials=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
case "$work" in /*) ;; *) echo 'Work directory must be absolute' >&2; exit 2 ;; esac

case "$mode" in
prepare)
    # Reserve source directories: never replace a recipient's modifications.
    mkdir "$work/runtime-source" "$work/fuse" "$work/squashfuse"
    tar -xf "$materials/type2-runtime-fedfad51.tar.gz" -C "$work/runtime-source" --strip-components=1
    tar -xf "$materials/fuse-3.15.0.tar.xz" -C "$work/fuse" --strip-components=1
    tar -xf "$materials/squashfuse-0.5.2.tar.gz" -C "$work/squashfuse" --strip-components=1
    (cd "$work/fuse" && patch -p1 < "$work/runtime-source/patches/libfuse/mount.c.diff")
    echo 'Prepared LGPL sources with the AppImage mount.c patch (2024-11-24).'
    ;;
build)
    # BuildKit owns image lifetime/cache; use its actual image ID, not a shared tag.
    docker build --platform linux/amd64 --iidfile "$work/builder-image-id" -f "$materials/Dockerfile" "$materials"
    docker run --rm --platform linux/amd64 \
        --mount "type=bind,src=$materials,dst=/materials,readonly" \
        --mount "type=bind,src=$work,dst=/work" \
        "$(cat "$work/builder-image-id")" sh /materials/rebuild.sh inside /work
    ;;
inside)
    # apk verifies official package signatures as well as the outer SHA-256 pins.
    apk add --no-network --allow-downgrades /materials/*.apk
    cd /work/fuse
    CFLAGS='-ffile-prefix-map=/work=.' meson setup --prefix=/usr --default-library=static build
    ninja -C build -v install
    cd /work/squashfuse
    export CFLAGS='-ffunction-sections -fdata-sections -Os -ffile-prefix-map=/work=.'
    ./autogen.sh
    ./configure LDFLAGS=-static
    make -j2
    make install
    install -m644 ./*.h /usr/local/include/squashfuse/
    cd /work/runtime-source/src/runtime
    {
        clang --version
        clang -print-libgcc-file-name
        apk info --installed --verbose
    } > /work/toolchain-packages.txt
    printf '%s\n' 'https://github.com/AppImage/type2-runtime/commit/fedfad5' > version
    make -B runtime CC='clang -Wl,-Map,/work/runtime-link.map'
    cp runtime /work/runtime-x86_64
    # Same AppImage magic initialization as the original build-runtime.sh.
    printf 'AI\002' | dd of=/work/runtime-x86_64 bs=1 count=3 seek=8 conv=notrunc
    ;;
*) echo 'Unknown rebuild mode' >&2; exit 2 ;;
esac
