use glam::Mat4;
use horizon_assets::{
    bake::bake_flat_face,
    model::{Direction, Material},
};
use horizon_client_render::{
    models::catalog::BakedFace,
    terrain_model::{NativeModelRenderOptions, draw_model},
};
use serde_json::{Value, json};
use std::io::{self, BufRead};
fn array<const N: usize>(value: &Value) -> [f32; N] {
    std::array::from_fn(|i| value[i].as_f64().unwrap() as f32)
}
fn main() {
    for line in io::stdin().lock().lines() {
        let q: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let direction = match q["direction"].as_str().unwrap() {
            "down" => Direction::Down,
            "up" => Direction::Up,
            "north" => Direction::North,
            "south" => Direction::South,
            "west" => Direction::West,
            "east" => Direction::East,
            _ => panic!(),
        };
        let mut quad = bake_flat_face(
            array(&q["from"]),
            array(&q["to"]),
            array(&q["uv"]),
            direction,
            Material {
                sprite: "fixture:sprite".into(),
                force_translucent: false,
            },
            -1,
        )
        .unwrap();
        // FaceBakery's quarter-turn UV selector, on this already baked rectangle.
        let turn = q["rotation"].as_u64().unwrap() as usize / 90;
        quad.uv.rotate_left(turn);
        let face = BakedFace {
            quad,
            layer: 0,
            opaque: true,
            translucent: false,
            fusion: None,
        };
        let mut out = Vec::new();
        assert!(draw_model(
            &[face],
            Mat4::from_cols_array(&array(&q["pose"])),
            0xf0f0,
            NativeModelRenderOptions {
                uv_shrink_ratio: 4.0 / q["atlas"].as_f64().unwrap() as f32,
                ..Default::default()
            },
            |_| Some(0),
            |v| {
                out.push(json!({"pos":v.pos,"uv":v.uv,"normal":[v.glint_uv[0],v.glint_uv[1],f32::from_ne_bytes(v.normal.map(|b|b as u8))]}))
            }
        ));
        println!("{}", serde_json::to_string(&out).unwrap());
    }
}
