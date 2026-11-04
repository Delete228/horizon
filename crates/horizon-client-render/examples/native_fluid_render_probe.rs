use horizon_client_render::fluid_render::{NativeFluidRenderOptions, fluid_box, fluid_light};
use serde_json::{Value, json};
use std::io::{self, BufRead};

fn main() {
    for line in io::stdin().lock().lines() {
        let input: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let bounds = std::array::from_fn(|i| input["bounds"][i].as_f64().unwrap() as f32);
        let argb = input["argb"].as_i64().unwrap() as u32;
        let native_light = ((input["light"].as_u64().unwrap() as u32 >> 16) & 0xff)
            | ((input["light"].as_u64().unwrap() as u32 & 0xff) << 8);
        let light = fluid_light(native_light, input["luminosity"].as_u64().unwrap() as u8);
        let java_light = ((light & 0xff) << 16) | (light >> 8 & 0xff);
        let mut vertices = Vec::new();
        assert!(fluid_box(
            bounds,
            NativeFluidRenderOptions {
                argb: Some(argb),
                render_bottom: input["bottom"].as_bool().unwrap(),
                uv_shrink_ratio: 4.0 / input["atlas"].as_u64().unwrap() as f32,
            },
            |v| vertices.push(v),
        ));
        let result: Vec<_> = vertices
            .into_iter()
            .map(|v| {
                json!({
                    "pos":v.pos,"uv":v.uv,"normal":v.normal,"light":java_light,
                    "rgba":[argb>>16&255,argb>>8&255,argb&255,argb>>24],
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&result).unwrap());
    }
}
