# Standard build utilities only; source and linked library inputs are mounted.
FROM alpine:3.21@sha256:ce64758a109eb420d874a118f87920e625e12d3634e03b4a5573fd9f6e5d3507
RUN apk add --no-cache bash alpine-sdk util-linux file autoconf automake libtool xz \
    eudev-dev gettext-dev linux-headers meson clang
