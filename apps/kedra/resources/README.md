# Native executable resources

`kedra.ico` is derived from the approved SVG at
`assets/editor/startup/project-mark-compact.svg`; it uses the same artwork.
The Windows build hook uses this checked-in icon for ordinary Cargo builds.
`cargo kedra` can supply freshly generated native inputs through
`KEDRA_WINDOWS_ICON`; the build hook never downloads or generates artwork.

Generation uses `cargo kedra icons` with the shared importer SVG rasterizer and
image crate's ICO encoder. Entries are 16, 32, 48, 64, 128 and 256 pixels; larger
PNG variants are used by the maintained native packager for macOS/Linux icons.

- SVG SHA-256: `d41b7e78e87afd7741d63281ad51c8e3b5a6e435dc83bb9394eb37cc50649a24`
- ICO SHA-256: `b27736f0d32197b96e6934e352ac207cdbc1d4d9735f517d3fec22e33bd1d1bb`

See [asset provenance](../../../assets/README.md) for the source details.
