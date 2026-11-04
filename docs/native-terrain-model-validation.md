# Explicit native terrain model rendering

`native_draw_terrain_model(draw, model, NativeModelRenderOptions)` draws a registered
standalone model under the normal block-entity/mob pose. It leaves
`native_draw_model` unchanged. The new material samples animated block sprite arrays,
including the current mip views; it does not use the shared GUI/entity atlas or Canvas.

```rust
game.native_draw_terrain_model(draw, "example:block/part", NativeModelRenderOptions {
    material: NativeModelMaterial::Model,
    shade: NativeModelShade::Model,
    uv_shift: None,
    uv_shrink_ratio: 0.0,
    tint: 0xffffff,
});
```

`Model` respects the baked solid/cutout/cutout-mipped material. Explicit `Solid`,
`Cutout` and `CutoutMipped` policies override it. Solid has no alpha discard;
cutout discards below .1 and uses no mipmaps; mipmapped cutout discards below .5.
All use back culling, ordinary depth comparison, color/depth writes and no blend
or model depth bias. Even alpha-zero solid pixels write color/depth. Unknown,
translucent and tripwire materials in `Model` mode fail before any geometry.
The baker now preserves an unknown declared material separately from an absent
default; ordinary terrain retains its previous pixel-based fallback.

`Model` shade respects the baked directional-shading flag, `Off` disables it,
and `Entity` explicitly uses the level's two entity light directions. Shading
is independent of block/sky light: fullbright shaded faces stay shaded. Flat
geometric normals are signed-byte quantized before inverse-transpose and
normalization, then preserved as floats in this material's private vertex payload.
The existing Vertex ABI is unchanged. Custom per-vertex smooth OBJ normals are
not represented by the current baker; this helper does not establish their parity.

`NativeModelUvShift` names an exact original and target sprite and supplies unit-sprite
scale/offset. Only matching named faces shift, even when identical textures share an
array layer. Missing sprites, a missing original-name match, invalid UV parameters,
nonfinite/singular poses and unsupported materials fail the whole call before its
first vertex. Sprite indices above 65,535 are rejected. The validation and streaming
passes use the draw's reused storage; they perform no registry scan or GPU allocation.

`uv_shrink_ratio` is an explicit older-FaceBakery compatibility policy, applied toward
the face UV bounds center before shifting. Zero preserves existing UVs. The actual
1.21.1 JSON FaceBakery uses `4 / max(stitchedAtlasWidth, stitchedAtlasHeight)`;
the isolated reference's 2048×1024 atlas needs `4 / 2048`. OBJ loaders that do not
shrink UVs must keep zero. This option does not infer a Java atlas from array dimensions.

`native_terrain_model_sprites(&mut Vec<String>)` registers block-sheet textures that no
model names, such as scrolling targets. It is asked once beside standalone model
registration. Every client resource load/reload decodes these names again; an absent
or invalid texture stays absent, so drawing cannot silently substitute missingno.
There is no directory scan or fake model/block registration. Server resource loads
do not decode these sprites.

The three opaque materials occupy new internal slots after the existing materials;
fluid, world Canvas, OIT and frame tails preserve their identities and order. Native
terrain UV/payloads remain on their own pipelines when a shader pack draws other
parts of the same entity span. They do not enter that pack's model/shadow pass.
Optional Horizon dynamic/colored lighting changes global lighting and is outside the
baseline comparison, as are shader-pack replacements and cross-version fog differences.

## Evidence

- The unchanged player-supplied Minecraft 1.21.1 `FaceBakery.bakeQuad`, including
  NeoForge normal fill, plus actual JOML instance transforms: **512 cases, 2,048
  vertices and 16,384 float values pass**. Six directions, quarter-turn UV rotations,
  negative/fractional bounds and UVs, four atlas sizes, rotated/nonuniform/mirrored
  poses are covered. Maximum absolute difference was 1.39e-6; comparison tolerances
  are explicitly 2e-5 absolute/2e-6 relative, not bit equality.
- Required actual Metal production shader/pipeline test: **pass**, with 15 pixels
  across four frames. Covers solid alpha-zero color/depth, .1/.5 cutoffs, no alpha
  blending, ordinary depth, mirrored-pose culling, mip views 3→1→3, unmipped cutout,
  fullbright directional/disabled/slope shading, Nether lights and lightmap tint.
  It is ignored in ordinary headless runs and must be explicitly executed with
  `--ignored`; it requires an adapter and has no successful early return.
- Initial full client no-default-feature library typecheck: pass.
- Frozen-source CPU checks: **6 pass**, including unsupported-input all-or-nothing
  validation, named-sprite matching, OIT payload preservation and registered-sprite
  reload/removal. Material batching checks: **7 pass**; unknown-material bake check:
  **1 passes**.
- Required actual Metal mixed-span routing test: **pass**, covering native fluid and
  all three new opaque materials alongside shader-pack model geometry, visible and
  hidden spans, and sprite bits that overlap the ordinary OIT marker.
- Strict Clippy for client, renderer, assets and generated API dispatch, all targets
  with no default features and warnings denied: **pass**. This is a focused result,
  not a full-workspace or full `pr_ready` claim.

The public-tree guard now explicitly allows only the three authored renderer
collector/fixture sources, while retaining the rejection of recovered Java, classes
and private resource trees. The inherited fluid oracle had not been included in the
earlier guard's exact allowlist: its earlier clean-tree result did not establish that
these subsequently tracked tool sources passed. The staged current tree and API9
revision range are checked separately after this correction.

Reproduce the source collector without launching a game:

```sh
python3 tools/test_native_terrain_model.py --reference-manifest /private/reference/manifest.json --javac /path/to/javac21-or-newer --output /private/scratch/model-oracle
cargo test -p horizon-client-render --lib native_model_material_mips_shade_culling_and_depth -- --ignored --nocapture
cargo test -p horizon-client-render --lib native_fluid_stays_native_while_model_uses_pack -- --ignored --nocapture
cargo test -p horizon-client-render --lib terrain_model
cargo test -p horizon-client-render --lib gfx::entity_batch::tests
cargo test -p horizon-assets --lib an_unknown_declared_render_type
cargo clippy -p horizon-client -p horizon-client-render -p horizon-assets -p horizon-mod-api-gen-tests --all-targets --no-default-features -- -D warnings
python3 -m unittest tests.test_lab.PublicationTests
```

The reference manifest supplies the player's existing 1.21.1 classpath; no original
classes, assets or decompiled source are bundled. Flywheel 1.0.6 source methods
`internal/diffuse.glsl`, `instance/transformed.vert` and `internal/common.vert`
specify directional shading/normal policy; the full MIT notice is in
`THIRD_PARTY_NOTICES.md`. This generic extension is not a burner feature or a
cross-client image comparison. Actual burner appearance requires its separate
source adapter, private resources and native/Java visual check.
