# Bomberarena: Explode Arena-inspired direction

Status: level blockout and gameplay proposal. Gameplay is not implemented.

## Reference research

Explode Arena is Infinite Dreams' 2004 Bomberman/Dynablaster-style game for
Symbian. Sources consulted on 2026-10-30:

- [All About Symbian release announcement, 19 July 2004](https://allaboutsymbian.com/news/item/New_Explode_Arena_from_Infinite_Dreams.php)
  reproduces the developer's feature list: more than 30 maps, single-player bots,
  Bluetooth multiplayer for up to four players, Deathmatch, Team Deathmatch,
  Last Man Standing, Capture the Flag and Domination. It also describes shadows,
  dynamic lighting, translucent explosions and fire/smoke/spark particles.
- [Gameplay screenshot accompanying that announcement](https://allaboutsymbian.com/news/images/explode2.jpg)
  shows sand-colored ground, orthogonal stone walls with long returns, open
  intersections, bombs and scattered pickups. This is evidence for one visual
  treatment, not proof that every map uses the same theme or layout.
- [Bohdan Dovhaliuk's retrospective, 21 February 2021](https://smart2000s.com/2021/02/21/explode-arena/)
  describes extra lives, additional bombs, blast-radius increases, speed,
  temporary passage through barriers, remote detonation and kicking bombs.
  These details are secondary-source reports, not independently tested rules.

The exact original map dimensions, movement quantization, explosion timing,
bonus durations and bot algorithms are not established by these sources.
Do not present proposed values below as original Explode Arena behavior.
No original game assets, binaries, music or level data are imported. The arena
uses the existing [KayKit sources and licenses](../../example/bomberarena/ASSETS.md)
and ordinary Kedra primitive geometry.

## Current authored arena

`example/bomberarena/assets/scenes/Arena.scene.ron` composes `ArenaLayout.prefab.ron`,
which mounts four copies of `CourtyardSector.prefab.ron` and one
`ApproachCrates.prefab.ron`. The scene, layout,
original nodes and existing scene-to-layout instance retain their identities.

- Playfield: 42 × 30 world units, designed as 21 × 15 cells with a two-unit pitch.
  This is a design grid; there is no runtime collision or navigation grid yet.
- Four starts: `(±18, ±12)` in X/Z, clear of walls and crates at the start marker.
- Four stone courtyards: long walls, wall returns, sheltered supply pockets and
  dense wooden blocks across otherwise free cells. Southern sectors rotate 180 degrees.
- Eight central stone pillars surround a low central objective marker.
- Twelve central blocks, thirty approach blocks and forty-one per mounted courtyard
  give 206 prospective destructible obstacles. Each sector defines 42 placements;
  each mount omits the one occupied by a central stone pillar. Their destruction
  behavior is not implemented.
- North/south and east/west paving marks the central routes. Their pillars require
  local detours. Both approaches and perimeter routes start blocked by crates;
  players are intended to excavate their routes during the round.
- Blue markers distinguish starts and supply locations from neutral stone and
  sand. Supply icons are static model previews, not collectible bonuses.
- The authored camera is raised to frame the enlarged arena. Scene View uses its
  own inspection camera; saved editor navigation can require Frame all.

All height differences are visual. Walls and crates do not yet block movement or
explosions. No bomb logic, objective scoring, Lua AI, Lua HUD, Play or following
game camera is supplied by this blockout. Current Scene View does not cast shadows.

## Object ownership

The files in `example/bomberarena/assets/prefabs/` own reusable object definitions:
`Crate`, `StoneWall`, `BoundaryWall`, `StonePillar`, `ObjectiveMarker`, `SpawnPad`,
`SupplyPad` and `PowerPickup`. Layout documents place object prefab instances directly,
with name and transform overrides on their roots, without per-object wrapper nodes.
Crate dimensions are defined in `Crate.prefab.ron`; crate instances override only
translation and rotation, inheriting scale without model or behavior overrides.
Geometry, future collision and future script components belong
to the owning object prefab so all linked instances inherit changes.

Sector and approach prefabs compose objects into reusable arrangements. The arena
composes these arrangements. The four sector omission overrides are layout choices,
not copied object definitions. Primitive sand, paving and foundation remain scenery.
No placeholder behavior components or new script-binding format are introduced;
gameplay scripts still require the runtime integration described below.

## Proposed gameplay

The defining direction is a large navigable battlefield with route choice,
destructible shortcuts and objective pressure, rather than a uniform pillar grid.

1. **Core loop:** four contestants, timed bombs, cross-shaped blast propagation,
   solid walls, destructible blocks and chain reactions. Begin with Last Man
   Standing to validate movement, escape opportunities and readable explosions.
2. **Bonuses:** bomb capacity, blast range and speed first; then kick and remote
   detonation, which make long corridors and corners tactically meaningful.
   Temporary barrier passage needs an explicit expiration/overlap rule before use.
3. **Large-map modes:** respawning Deathmatch, then central Domination. CTF needs
   distinct team bases, flag rules and route balancing; the current start pads
   are not working CTF bases.
4. **Lua AI:** assess time-to-explosion, reserve an escape route before placing a
   bomb, use alternate routes and pursue objectives. Implement through accepted
   game/runtime capabilities; editor preview is not a gameplay host.
5. **Lua HUD:** round timer, score, bomb capacity, range and active bonus state.
   Keep controls and state compact; do not place design explanations in the HUD.
6. **Camera:** an elevated player-following view with enough look-ahead to read
   escape routes. A full-map overview is useful for inspection but is not the
   proposed normal playing view.

The next implementation boundary is an approved gameplay/runtime integration path,
consistent with [the Scene/Game runtime plan](scene-game-runtime.md). Blockout
geometry alone does not establish navigation fairness or gameplay acceptance.

## Acceptance targets

- Every start has at least two immediate escape choices before any block breaks.
- After destructible blocks are cleared, all starts and supply pockets are connected
  without crossing solid walls.
- Each start has a small clear maneuvering pocket; leaving it requires breaking blocks.
- A bomb at an ordinary junction has counterplay; safe paths must be evaluated
  against timing and the actual player footprint, not just point connectivity.
- Native gameplay tests exercise chain reactions, collision, input and Lua
  behavior once those systems exist. Import/build verification only proves asset
  delivery and scene validity; renderer captures prove appearance, not gameplay.
