# Bomberarena

A 3D arena blockout inspired by Explode Arena's stone labyrinths, larger battlefields
and objective-based modes, assembled from the existing CC0 KayKit models.

Open `assets/scenes/Arena.scene.ron` in Kedra. The 42 × 30 unit courtyard contains
four start markers, four stone sectors, 206 crate instances, supply previews and a
central objective marker. Use Scene View **Frame all** if a saved inspection camera
is still zoomed into part of the map.

This is authored scenery: collision, destructible blocks, bombs, pickups, Lua AI,
Lua HUD, gameplay camera and Play are not implemented.

## Reusable objects

Edit `assets/prefabs/Crate.prefab.ron` to change the shared crate model or dimensions.
All 206 crates are linked instances, not independent mesh copies. Place future
collision and script components on the object prefab, not separately on each copy.
Objects are direct prefab instances with root name/transform overrides, without
per-object wrapper nodes. Crates override translation and rotation and inherit scale.
The other reusable definitions are `StoneWall`, `BoundaryWall`, `StonePillar`,
`SpawnPad`, `SupplyPad`, `PowerPickup` and `ObjectiveMarker` in the same directory.

`CourtyardSector.prefab.ron` and `ApproachCrates.prefab.ron` own arrangements;
`ArenaLayout.prefab.ron` composes them. Layouts own positions/orientations and wall
lengths. Each courtyard mount omits one crate where the central pillar occupies
its cell. Clear spawn pockets are small; the routes start densely filled with crates.

- [Research, layout and gameplay proposal](../../docs/plans/bomberarena.md)
- [Asset provenance and licenses](ASSETS.md)

Build a fresh delivery after editing scene or prefab sources:

```sh
cargo run -p kedra -- assets init example/bomberarena/assets
cargo run -p kedra -- assets build example/bomberarena/assets target/bomberarena-delivery --namespace game
```

The output directory must be new. Launch the editor with that directory as
`--project-assets`, in addition to the normal verified editor `--assets` delivery.
