//! Actual immutable context-table import, shape queries, merged edges and clipping.
use horizon_core::{
    geometry::{Aabb, Axis, ShapeOp, Vec3, VoxelShape},
    position::BlockPos,
};
use horizon_world::{
    collision::CachedCollisionRegistry,
    collision_context::EntityContextKind,
    nbt::{Compound, Tag},
    registry::{BlockRegistry, BlockStateId},
};
use serde_json::Value;
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};
fn shape(v: &Value) -> VoxelShape {
    v.as_array()
        .unwrap()
        .iter()
        .fold(VoxelShape::empty(), |s, b| {
            let v: Vec<_> = b
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_f64().unwrap())
                .collect();
            s.join(
                &VoxelShape::from_box(Aabb::new(
                    Vec3::new(v[0], v[1], v[2]),
                    Vec3::new(v[3], v[4], v[5]),
                ))
                .unwrap(),
                ShapeOp::OR,
            )
            .unwrap()
        })
}
fn encode(shape: &VoxelShape) -> Tag {
    let mut c = Compound::new();
    let (min, max) = shape.discrete_bounds();
    c.insert("canonical".into(), Tag::Byte(0));
    c.insert("cube".into(), Tag::Byte(i8::from(shape.is_cube())));
    let axes = shape.cube_coordinate_axes();
    c.insert(
        "cube_axes".into(),
        Tag::Byte(i8::from(axes[0]) | i8::from(axes[1]) << 1 | i8::from(axes[2]) << 2),
    );
    for (name, axis) in ["x", "y", "z"].into_iter().zip(Axis::ALL) {
        c.insert(
            name.into(),
            Tag::LongArray(
                shape
                    .coordinates(axis)
                    .iter()
                    .map(|n| n.to_bits() as i64)
                    .collect(),
            ),
        );
    }
    c.insert(
        "occupied".into(),
        Tag::ByteArray({
            let [nx, ny, nz] = shape.sizes();
            (0..nx)
                .flat_map(|x| {
                    (0..ny).flat_map(move |y| {
                        (0..nz).map(move |z| u8::from(shape.is_full(x as i32, y as i32, z as i32)))
                    })
                })
                .collect()
        }),
    );
    c.insert("discrete_min".into(), Tag::IntArray(min.to_vec()));
    c.insert("discrete_max".into(), Tag::IntArray(max.to_vec()));
    Tag::Compound(c)
}
fn values(out: &mut impl Write, label: &str, v: impl IntoIterator<Item = f64>) -> io::Result<()> {
    write!(out, "{label}")?;
    for x in v {
        write!(out, " {:016x}", x.to_bits())?;
    }
    writeln!(out)
}
fn vec(v: &Value) -> Vec3 {
    Vec3::new(
        v[0].as_str().unwrap().parse().unwrap(),
        v[1].as_str().unwrap().parse().unwrap(),
        v[2].as_str().unwrap().parse().unwrap(),
    )
}
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = PathBuf::from(super::args().nth(1).ok_or("directory")?);
    let mut out = io::BufWriter::new(fs::File::create(dir.join("rust.txt"))?);
    for line in fs::read_to_string(dir.join("inputs.txt"))?.lines() {
        let f: Value = serde_json::from_str(line)?;
        let states = Tag::List(vec![Tag::compound([(
            "id",
            Tag::String("horizon:shape".into()),
        )])]);
        let registry = BlockRegistry::from_nbt(&Tag::compound([
            ("DataVersion", Tag::Int(horizon_world::DATA_VERSION)),
            ("states", states.clone()),
            ("defaults", Tag::compound([("horizon:shape", Tag::Int(0))])),
        ]))?;
        let default = shape(&f["default"]);
        let mut shapes = vec![encode(&default)];
        let mut row = Compound::from([("state".into(), Tag::Int(0))]);
        for key in [
            "outline_item",
            "outline_player",
            "outline_living",
            "outline_other",
            "collision_item",
            "collision_player",
            "collision_living",
            "collision_other",
            "interaction",
        ] {
            if let Some(v) = f.get(key) {
                row.insert(key.into(), Tag::Int(shapes.len() as i32));
                shapes.push(encode(&shape(v)));
            }
        }
        let table = Tag::compound([
            ("DataVersion", Tag::Int(horizon_world::DATA_VERSION)),
            ("states", states),
            ("shapes", Tag::List(shapes)),
            ("cached_shapes", Tag::IntArray(vec![0])),
            ("context_shapes", Tag::List(vec![Tag::Compound(row)])),
        ]);
        let cached = CachedCollisionRegistry::from_nbt(&table, &registry)?;
        let kind = match f["kind"].as_str().unwrap() {
            "player" => EntityContextKind::Player,
            "item" => EntityContextKind::Item,
            "living" => EntityContextKind::Living,
            "other" => EntityContextKind::Other,
            _ => EntityContextKind::None,
        };
        let outline = cached
            .context_outline_shape(BlockStateId(0), kind)
            .unwrap_or(&default);
        let collision = cached
            .context_collision_shape(BlockStateId(0), kind)
            .unwrap_or(&default);
        for (label, s) in [("outline", outline), ("collision", collision)] {
            writeln!(out, "{label} {}", s.boxes().len())?;
            for b in s.boxes() {
                values(
                    &mut out,
                    "box",
                    Axis::ALL
                        .map(|a| b.min(a))
                        .into_iter()
                        .chain(Axis::ALL.map(|a| b.max(a))),
                )?;
            }
            let edges = s.edges();
            writeln!(out, "edges {}", edges.len())?;
            for e in edges {
                values(&mut out, "edge", e)?;
            }
        }
        let bounds = &f["bounds"];
        let b = Aabb::new(vec(&bounds[0]), vec(&bounds[1]));
        values(
            &mut out,
            "move",
            [collision.collide(Axis::Y, b, f["dy"].as_f64().unwrap())],
        )?;
        let start = vec(&f["from"]);
        let end = vec(&f["to"]);
        if let Some(hit) = horizon_world::interaction_shapes::clip_with_interaction_override(
            outline.boxes().iter().copied(),
            cached
                .interaction_shape(BlockStateId(0))
                .into_iter()
                .flat_map(|s| s.boxes().iter().copied()),
            start,
            end,
            BlockPos::new(0, 0, 0),
        ) {
            writeln!(out, "hit {} {}", hit.direction as usize, hit.inside)?;
            values(
                &mut out,
                "point",
                [hit.location.x, hit.location.y, hit.location.z],
            )?;
        } else {
            writeln!(out, "miss")?;
        }
    }
    Ok(())
}
