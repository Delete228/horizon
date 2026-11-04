# Native still-fluid rendering

The helper draws known still fluids under a native block entity or mob pose. It uses the
animated block sprite arrays, their current mip views and the normal world lightmap/fog.
It does not use the GUI atlas, translucent-item material or world canvas.

```rust
game.native_draw_fluid_box(draw, &stack, [x0, y0, z0, x1, y1, z1], NativeFluidRenderOptions {
    argb: None,
    render_bottom: false,
    uv_shrink_ratio: 4.0 / 2048.0,
});
```

Supported IDs are `minecraft:water` and `minecraft:lava` with an empty component patch.
Unknown IDs and every nonempty patch (including removals) return false before drawing.
Empty accepted amounts are a successful no-op without validating bounds. Nonempty stacks
reject nonfinite/reversed bounds, coordinates outside ±1,048,576 or more than 65,536 tiled
quads before drawing. No accepted box is silently truncated. The geometry streams into the
draw's reused vertex storage; each draw needs an O(1) sprite lookup and no GPU allocations.

Water uses the stack tint `0xff3f76e4`, independently of biome color; lava uses white.
`argb` is an explicit override. Lava raises block light to 15 while preserving sky light.
The helper does not guess how components or unknown fluids affect texture, tint or luminosity.

`uv_shrink_ratio` is explicit because Horizon uses sprite arrays. The source value is
`4 / max(stitchedBlockAtlasWidth, stitchedBlockAtlasHeight)`. The isolated Create 6.0.10
reference produced a 2048×1024 block atlas, hence `4 / 2048` for that fixture. A different
stitched reference atlas needs its own value; this parameter does not reconstruct Java's
atlas layout from Horizon's array dimensions.

The material sorts quads far to near, uses nearest texels with linear mip selection,
alpha blends, culls backs and writes ordinary color and depth. There is no depth bias,
model diffuse shade, geometry inset or alpha discard: even alpha-zero texels write depth.
It draws before terrain transparency, outside OIT queues. Public world-canvas overlays,
OIT/item slots and frame tails retain their previous identities. A shader pack can still
draw the model in a mixed model/fluid block entity run; fluid remains on its own material
because its UVs address block arrays, not the shared entity atlas. Resource reload builds
new bindings; changing mip levels rebuilds the fluid bindgroup with the current views.

Baseline comparisons disable Horizon's optional dynamic and colored lights. Enabling those
extras changes global world lighting, which the fluid shader inherits too. Neither the
method oracle nor a synthetic GPU test establishes cross-client pixel equality.

## Source

The player's Create `1.21.1-6.0.10.jar` embeds
`net.createmod.ponder:ponder-neoforge:1.0.82+mc1.21.1`, SHA-256
`0cf4611ad853042b689ac386184c5bbe02950efcffddb49e5f604e82baddb0dc`.

- `FluidRenderHelper.renderFluidBox`, `renderStillTiledFace`, `renderTiledFace`, `putVertex`:
  face order/winding, integer tiling, directional UV mirrors, source UV shrink, raw ARGB,
  luminosity and sky-light preservation.
- `NeoForgeFluidHelper.getColor/getLuminosity/getStillTexture`: stack tint, fluid-type
  luminosity and still sprite, with no biome lookup for a stack rendered without a world/position.
- `PonderRenderTypes.FLUID`: BLOCK/QUADS, sort-on-upload, translucent shader/blend,
  mipmapped block sheet/lightmap and default cull/depth/color-depth write state.
- Pinned Minecraft 1.21.1 `TextureAtlasSprite.uvShrinkRatio`,
  `AbstractTexture.setFilter(false,true)`, `rendertype_translucent.vsh/fsh` and
  `LevelRenderer.renderLevel`: shrink definition, nearest/linear-mip filtering,
  unshaded lightmap tint, no alpha cutoff and custom-buffer flush before translucent terrain.

Catnip/Ponder is MIT; attribution and the full license accompany `fluid_render.rs`.
The Java oracle executes the unchanged player-supplied helper. A small service fixture
supplies explicit sprite/color/luminosity without starting a game/mod loader. It does not
copy the source algorithm into the expected result. No game assets/classes are bundled.

## Checks

- Actual unchanged Catnip helper: **2,062 boxes, 179,784 vertices pass**, comparing f32
  position/normalized in-sprite UVs, normals, stack RGBA and light. Includes negative,
  integer, fractional, zero-height/all-zero bounds, both bottom policies and four atlas sizes.
  Run `tools/test_native_fluid_render.py --reference-manifest <private-manifest> --javac <javac21+>`.
- Full client no-default-feature library/binary typecheck: pass. Focused render tests: nine pass,
  including source geometry/light and unchanged Canvas/OIT/frame-tail grouping.
- Actual Metal production shader/pipeline readback: pass. Checks nearest texels/linear mip
  selection, mip view reconstruction 3→1→3, raw tint/light, alpha-zero depth writes,
  ordinary depth without model bias, negative-X mirrored-pose culling and alpha blending.
- Actual Metal mixed-model/fluid shader-pack prepare regression: pass. The ordinary model
  still uses its pack/shadow route; fluid retains its own UV/payload/material.
- Strict client/render all-target Clippy with warnings denied: pass. After hardening the
  GPU test to require an adapter, render all-target Clippy passes again. Both targeted Metal
  checks require a device and have no successful early-return path.
- Full `pr_ready --base cd46023a4f5d14ea3423f5344a1b2617c9a0af2d` completed in
  61 minutes 31 seconds: 3,822 touched-crate tests, documentation tests, public-tree,
  formatting and strict transitive all-target Clippy passed. Launcher and content-root
  checks were not selected; vanilla parity was not run because this checkout has no
  prepared vanilla data. The Python run reported three failures among 634 tests (one
  skipped): two inherited fluid-capability driver convention failures, corrected in
  local follow-up `68261dc556e8a2fe572cf26a64d7dcc717be28ed`, and the independently
  reproduced unchanged-base Darwin installer fixture failure described in
  `native-fluid-capabilities-validation.md`. The complete aggregate check is not
  claimed as a pass.
- Basin cross-client appearance and resource-pack integration: separate native feature check.
