# KayKit source assets

Creator: **Kay Lousberg**, <https://kaylousberg.com/>.
Both source packs are licensed under **CC0 1.0**:
<https://creativecommons.org/publicdomain/zero/1.0/>.
The original notices are included verbatim:

- [Platformer Pack 1.0 FREE](licenses/KayKit_Platformer_Pack_1.0_FREE/License.txt)
- [Adventurers Character Pack 2.0 FREE](licenses/KayKit_Adventurers_2.0_FREE/License.txt)

## Representation

Platformer uses one blue canonical model per colored shape plus the neutral
models. All 83 four-color groups have matching triangle positions/normals to
1e-5 after accounting for export vertex order. Colors occupy different UV regions
of the same atlas; multiplying a material tint is not an equivalent replacement.
The canonical geometry references the shared `textures/platformer_texture.png`.
Alternative export formats and repeated color meshes are not imported.

Adventurers includes accessories, six characters and both supplied animation
libraries. GLB conversion retains animation, skin and mesh counts. Unique textures
are shared externally rather than embedded repeatedly in every model.

Conversion uses glTF Pipeline 4.3.1 (Apache-2.0), with unused elements retained and
no lossy compression. glTF Transform 4.5.1 (MIT) checks parsed output and geometry.
Existing imported Platformer UUIDs are retained when converting their incomplete
JSON glTF files to GLB. Original imports are retained outside active assets in a
local conversion recovery directory. Original download archives remain unchanged.

Binary assets use Git LFS; `.meta` UUIDs and these notices are ordinary Git text.
The native importer currently disables animation loading. Retained animation data
does not imply animated preview, retargeting, gameplay, or live material-variant
controls. Preview requires a matching project delivery.
