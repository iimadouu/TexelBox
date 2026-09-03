use image::RgbaImage;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
#[derive(Debug, thiserror::Error)]
pub enum AtlasError {
    #[error("no images to pack")]
    Empty,
    #[error("invalid atlas parameters: {0}")]
    InvalidParams(String),
    #[error("images do not fit into a {0}×{0} atlas — reduce padding, remove images or enable rotation packing (Pro)")]
    DoesNotFit(u32),
}
pub const FREE_MAX_IMAGES: usize = 32;
pub const FREE_MAX_SIZE: u32 = 2048;
pub const PRO_MAX_SIZE: u32 = 8192;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SidecarFormat {
    Json,
    Xml,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AtlasParams {
    pub max_size: u32,
    pub padding: u32,
    pub bleed: u32,
    pub allow_rotation: bool,
    pub sidecar: SidecarFormat,
    pub trim_sheet: bool,
    pub priority_arrange: bool,
}
impl Default for AtlasParams {
    fn default() -> Self {
        Self { max_size: 2048, padding: 2, bleed: 0, allow_rotation: false, sidecar: SidecarFormat::Json, trim_sheet: false, priority_arrange: false }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AtlasTile {
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub rotated: bool,
    pub uv: [f32; 4],
}
pub struct AtlasInput<'a> {
    pub name: &'a str,
    pub image: &'a RgbaImage,
    pub priority: u8,
}
pub struct AtlasResult {
    pub size: u32,
    pub image: RgbaImage,
    pub tiles: Vec<AtlasTile>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Rect {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}
impl Rect {
    fn intersects(&self, o: &Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }
    fn contains(&self, o: &Rect) -> bool {
        o.x >= self.x && o.y >= self.y && o.x + o.w <= self.x + self.w && o.y + o.h <= self.y + self.h
    }
}
struct Packer {
    free: Vec<Rect>,
}
struct Placement {
    x: u32,
    y: u32,
    rotated: bool,
}
impl Packer {
    fn new(size: u32, gap: u32) -> Self {
        Self { free: vec![Rect { x: gap, y: gap, w: size.saturating_sub(gap), h: size.saturating_sub(gap) }] }
    }
    fn find(&self, w: u32, h: u32, allow_rotation: bool) -> Option<Placement> {
        let mut best: Option<(i64, i64, Placement)> = None;
        let mut consider = |fw: u32, fh: u32, rotated: bool| {
            for r in &self.free {
                if fw <= r.w && fh <= r.h {
                    let lw = (r.w - fw) as i64;
                    let lh = (r.h - fh) as i64;
                    let score = (lw.min(lh), lw.max(lh));
                    let better = match &best {
                        None => true,
                        Some((s0, s1, _)) => score.0 < *s0 || (score.0 == *s0 && score.1 < *s1),
                    };
                    if better {
                        best = Some((score.0, score.1, Placement { x: r.x, y: r.y, rotated }));
                    }
                }
            }
        };
        consider(w, h, false);
        if allow_rotation && w != h {
            consider(h, w, true);
        }
        best.map(|(_, _, p)| p)
    }
    fn place(&mut self, used: Rect) {
        let mut i = 0;
        while i < self.free.len() {
            if !self.free[i].intersects(&used) {
                i += 1;
                continue;
            }
            let f = self.free.remove(i);
            if used.x > f.x {
                self.free.push(Rect { x: f.x, y: f.y, w: used.x - f.x, h: f.h });
            }
            if used.x + used.w < f.x + f.w {
                self.free.push(Rect { x: used.x + used.w, y: f.y, w: (f.x + f.w) - (used.x + used.w), h: f.h });
            }
            if used.y > f.y {
                self.free.push(Rect { x: f.x, y: f.y, w: f.w, h: used.y - f.y });
            }
            if used.y + used.h < f.y + f.h {
                self.free.push(Rect { x: f.x, y: used.y + used.h, w: f.w, h: (f.y + f.h) - (used.y + used.h) });
            }
        }
        let mut keep = vec![true; self.free.len()];
        for i in 0..self.free.len() {
            if !keep[i] {
                continue;
            }
            for j in 0..self.free.len() {
                if i == j || !keep[j] || !self.free[j].contains(&self.free[i]) {
                    continue;
                }
                if self.free[j] != self.free[i] || j < i {
                    keep[i] = false;
                    break;
                }
            }
        }
        let mut idx = 0;
        self.free.retain(|_| {
            let k = keep[idx];
            idx += 1;
            k
        });
    }
}
fn rotate90_cw(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    let mut out = RgbaImage::new(h, w);
    for sy in 0..h {
        for sx in 0..w {
            out.put_pixel(h - 1 - sy, sx, *img.get_pixel(sx, sy));
        }
    }
    out
}
fn expand_edges(img: &RgbaImage, b: u32) -> RgbaImage {
    if b == 0 {
        return img.clone();
    }
    let (w, h) = img.dimensions();
    let (ow, oh) = (w + 2 * b, h + 2 * b);
    let src = img.as_raw();
    let mut out = vec![0u8; (ow as usize) * (oh as usize) * 4];
    out.par_chunks_mut(4)
        .enumerate()
        .for_each(|(i, px)| {
            let ox = (i % ow as usize) as i64;
            let oy = (i / ow as usize) as i64;
            let sx = (ox - b as i64).clamp(0, w as i64 - 1) as usize;
            let sy = (oy - b as i64).clamp(0, h as i64 - 1) as usize;
            px.copy_from_slice(&src[(sy * w as usize + sx) * 4..(sy * w as usize + sx) * 4 + 4]);
        });
    RgbaImage::from_raw(ow, oh, out).expect("expand_edges: buffer size is exact")
}
fn blit(dst: &mut RgbaImage, dx: u32, dy: u32, src: &RgbaImage) {
    let (sw, sh) = src.dimensions();
    let (dw, dh) = dst.dimensions();
    assert!(dx + sw <= dw && dy + sh <= dh, "atlas blit out of bounds");
    let src_raw = src.as_raw();
    let flat = dst.as_flat_samples_mut();
    let dst_raw = flat.samples;
    for row in 0..sh {
        let s_from = (row * sw * 4) as usize;
        let d_from = (((dy + row) * dw + dx) * 4) as usize;
        let len = (sw * 4) as usize;
        dst_raw[d_from..d_from + len].copy_from_slice(&src_raw[s_from..s_from + len]);
    }
}
fn trim_inputs<'a>(inputs: &[AtlasInput<'a>]) -> Vec<RgbaImage> {
    let cell = inputs
        .iter()
        .map(|i| {
            let (w, h) = i.image.dimensions();
            w.max(h)
        })
        .max()
        .unwrap_or(0);
    inputs
        .iter()
        .map(|i| {
            let (w, h) = i.image.dimensions();
            let largest = w.max(h);
            if largest == 0 || largest == cell {
                return i.image.clone();
            }
            let scale = cell as f64 / largest as f64;
            let nw = ((w as f64 * scale).round() as u32).max(1);
            let nh = ((h as f64 * scale).round() as u32).max(1);
            image::imageops::resize(i.image, nw, nh, image::imageops::FilterType::Lanczos3)
        })
        .collect()
}
pub fn pack(inputs: &[AtlasInput<'_>], params: &AtlasParams) -> Result<AtlasResult, AtlasError> {
    if inputs.is_empty() {
        return Err(AtlasError::Empty);
    }
    if !params.max_size.is_power_of_two() || !(16..=PRO_MAX_SIZE).contains(&params.max_size) {
        return Err(AtlasError::InvalidParams(format!(
            "max_size must be a power of two between 16 and {PRO_MAX_SIZE}"
        )));
    }
    if params.padding > 64 {
        return Err(AtlasError::InvalidParams("padding must be <= 64".into()));
    }
    if params.bleed > 64 {
        return Err(AtlasError::InvalidParams("bleed must be <= 64".into()));
    }
    let trimmed: Vec<RgbaImage>;
    let items: Vec<(&str, &RgbaImage)> = if params.trim_sheet {
        trimmed = trim_inputs(inputs);
        inputs.iter().zip(trimmed.iter()).map(|(i, t)| (i.name, t)).collect()
    } else {
        inputs.iter().map(|i| (i.name, i.image)).collect()
    };
    for (name, img) in &items {
        let (w, h) = img.dimensions();
        if w == 0 || h == 0 {
            return Err(AtlasError::InvalidParams(format!("image '{name}' is empty")));
        }
    }
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|a, b| {
        let ea = items[*a].1.dimensions();
        let eb = items[*b].1.dimensions();
        if params.priority_arrange {
            let pa = inputs[*a].priority;
            let pb = inputs[*b].priority;
            pa.cmp(&pb).then_with(|| eb.0.max(eb.1).cmp(&ea.0.max(ea.1)))
        } else {
            eb.0.max(eb.1).cmp(&ea.0.max(ea.1))
        }
    });
    let gap = params.padding.max(params.bleed);
    let mut size = 16u32;
    'sizes: loop {
        if size > params.max_size {
            return Err(AtlasError::DoesNotFit(params.max_size));
        }
        let mut packer = Packer::new(size, gap);
        let mut placements: Vec<(usize, Placement)> = Vec::with_capacity(order.len());
        for idx in &order {
            let (w, h) = items[*idx].1.dimensions();
            let Some(p) = packer.find(w + gap, h + gap, params.allow_rotation) else {
                size *= 2;
                continue 'sizes;
            };
            let (uw, uh) = if p.rotated { (h + gap, w + gap) } else { (w + gap, h + gap) };
            packer.place(Rect { x: p.x, y: p.y, w: uw, h: uh });
            placements.push((*idx, p));
        }
        placements.sort_by_key(|(idx, _)| *idx);
        let mut canvas = RgbaImage::new(size, size);
        let mut tiles: Vec<AtlasTile> = Vec::with_capacity(placements.len());
        for (idx, p) in placements {
            let (name, img) = items[idx];
            let (w, h) = img.dimensions();
            let sprite = if p.rotated { rotate90_cw(img) } else { img.clone() };
            let with_bleed = expand_edges(&sprite, params.bleed);
            let tx = p.x;
            let ty = p.y;
            blit(&mut canvas, tx.saturating_sub(params.bleed), ty.saturating_sub(params.bleed), &with_bleed);
            let (rw, rh) = if p.rotated { (h, w) } else { (w, h) };
            let s = size as f32;
            tiles.push(AtlasTile {
                name: name.to_string(),
                x: tx,
                y: ty,
                width: w,
                height: h,
                rotated: p.rotated,
                uv: [tx as f32 / s, ty as f32 / s, (tx + rw) as f32 / s, (ty + rh) as f32 / s],
            });
        }
        return Ok(AtlasResult { size, image: canvas, tiles });
    }
}
pub fn sidecar_json(result: &AtlasResult) -> String {
    let tiles: Vec<serde_json::Value> = result
        .tiles
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "x": t.x, "y": t.y,
                "width": t.width, "height": t.height,
                "rotated": t.rotated,
                "regionWidth": if t.rotated { t.height } else { t.width },
                "regionHeight": if t.rotated { t.width } else { t.height },
                "uv": { "u0": t.uv[0], "v0": t.uv[1], "u1": t.uv[2], "v1": t.uv[3] },
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({
        "format": "texelbox-atlas/1",
        "atlas": { "width": result.size, "height": result.size },
        "tileCount": result.tiles.len(),
        "tiles": tiles,
    }))
    .expect("atlas sidecar JSON always serializes")
}
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
pub fn sidecar_xml(result: &AtlasResult) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    out.push_str(&format!(
        "<atlas format=\"texelbox-atlas/1\" width=\"{}\" height=\"{}\" tileCount=\"{}\">\n",
        result.size,
        result.size,
        result.tiles.len()
    ));
    for t in &result.tiles {
        out.push_str(&format!(
            "  <tile name=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rotated=\"{}\" regionWidth=\"{}\" regionHeight=\"{}\" u0=\"{:.6}\" v0=\"{:.6}\" u1=\"{:.6}\" v1=\"{:.6}\"/>\n",
            xml_escape(&t.name),
            t.x,
            t.y,
            t.width,
            t.height,
            t.rotated,
            if t.rotated { t.height } else { t.width },
            if t.rotated { t.width } else { t.height },
            t.uv[0],
            t.uv[1],
            t.uv[2],
            t.uv[3],
        ));
    }
    out.push_str("</atlas>\n");
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;
    fn solid(w: u32, h: u32, c: Rgba<u8>) -> RgbaImage {
        RgbaImage::from_pixel(w, h, c)
    }
    fn red() -> Rgba<u8> {
        Rgba([200, 30, 30, 255])
    }
    fn green() -> Rgba<u8> {
        Rgba([30, 200, 30, 255])
    }
    fn blue() -> Rgba<u8> {
        Rgba([30, 30, 200, 255])
    }
    #[test]
    fn packs_two_tiles_into_smallest_pot() {
        let a = solid(8, 8, red());
        let b = solid(8, 8, green());
        let inputs = [
            AtlasInput { name: "a", image: &a, priority: 3 },
            AtlasInput { name: "b", image: &b, priority: 3 },
        ];
        let params = AtlasParams { max_size: 2048, padding: 2, bleed: 0, allow_rotation: false, sidecar: SidecarFormat::Json, trim_sheet: false, priority_arrange: false };
        let r = pack(&inputs, &params).unwrap();
        assert_eq!(r.size, 32, "two 8×8 tiles + padding fit in 32², not larger");
        assert_eq!(r.tiles.len(), 2);
        for t in &r.tiles {
            assert!(t.x + t.width <= r.size && t.y + t.height <= r.size);
        }
        let (t0, t1) = (&r.tiles[0], &r.tiles[1]);
        let overlap_x = t0.x < t1.x + t1.width && t1.x < t0.x + t0.width;
        let overlap_y = t0.y < t1.y + t1.height && t1.y < t0.y + t0.height;
        assert!(!(overlap_x && overlap_y), "tiles must not overlap");
        assert_eq!(*r.image.get_pixel(t0.x, t0.y), red());
        assert_eq!(*r.image.get_pixel(t1.x, t1.y), green());
        assert!((t0.uv[0] - 2.0 / 32.0).abs() < 1e-6);
        assert!((t0.uv[2] - 10.0 / 32.0).abs() < 1e-6);
    }
    #[test]
    fn fails_when_too_large_for_max_size() {
        let big = solid(32, 32, red());
        let inputs = [AtlasInput { name: "big", image: &big, priority: 3 }];
        let params = AtlasParams { max_size: 32, padding: 2, ..AtlasParams::default() };
        assert!(matches!(pack(&inputs, &params), Err(AtlasError::DoesNotFit(32))));
        let params = AtlasParams { max_size: 64, ..params };
        assert!(pack(&inputs, &params).is_ok());
    }
    #[test]
    fn bleed_extends_edge_pixels() {
        let a = solid(4, 4, red());
        let inputs = [AtlasInput { name: "a", image: &a, priority: 3 }];
        let params = AtlasParams { max_size: 64, padding: 4, bleed: 2, allow_rotation: false, sidecar: SidecarFormat::Json, trim_sheet: false, priority_arrange: false };
        let r = pack(&inputs, &params).unwrap();
        let t = &r.tiles[0];
        assert_eq!((t.x, t.y), (4, 4), "gap offset is max(padding, bleed)");
        assert_eq!(*r.image.get_pixel(t.x, t.y), red());
        assert_eq!(*r.image.get_pixel(t.x - 1, t.y), red());
        assert_eq!(*r.image.get_pixel(t.x - 2, t.y + 1), red());
        assert_eq!(*r.image.get_pixel(t.x + 1, t.y - 2), red());
        assert_eq!(*r.image.get_pixel(t.x - 3, t.y), Rgba([0, 0, 0, 0]));
    }
    #[test]
    fn rotation_packs_what_cannot_fit_unrotated() {
        let w1 = solid(14, 4, red());
        let w2 = solid(14, 4, green());
        let tall = solid(6, 12, blue());
        let inputs = [
            AtlasInput { name: "w1", image: &w1, priority: 3 },
            AtlasInput { name: "w2", image: &w2, priority: 3 },
            AtlasInput { name: "tall", image: &tall, priority: 3 },
        ];
        let base = AtlasParams { max_size: 16, padding: 0, bleed: 0, allow_rotation: false, sidecar: SidecarFormat::Json, trim_sheet: false, priority_arrange: false };
        assert!(matches!(pack(&inputs, &base), Err(AtlasError::DoesNotFit(16))));
        let params = AtlasParams { allow_rotation: true, ..base };
        let r = pack(&inputs, &params).unwrap();
        let tall_tile = r.tiles.iter().find(|t| t.name == "tall").unwrap();
        assert!(tall_tile.rotated);
        assert_eq!((tall_tile.width, tall_tile.height), (6, 12), "tile dims stay source-oriented");
        assert_eq!(*r.image.get_pixel(tall_tile.x, tall_tile.y), blue());
    }
    #[test]
    fn trim_sheet_normalizes_cell_size() {
        let small = solid(8, 4, red());
        let big = solid(16, 2, green());
        let inputs = [
            AtlasInput { name: "small", image: &small, priority: 3 },
            AtlasInput { name: "big", image: &big, priority: 3 },
        ];
        let params = AtlasParams { max_size: 128, padding: 2, bleed: 0, allow_rotation: false, sidecar: SidecarFormat::Json, trim_sheet: true, priority_arrange: false };
        let r = pack(&inputs, &params).unwrap();
        let small_tile = r.tiles.iter().find(|t| t.name == "small").unwrap();
        let big_tile = r.tiles.iter().find(|t| t.name == "big").unwrap();
        assert_eq!(big_tile.width.max(big_tile.height), 16);
        assert_eq!(small_tile.width.max(small_tile.height), 16);
        assert_eq!((small_tile.width, small_tile.height), (16, 8), "aspect preserved");
    }
    #[test]
    fn priority_arrange_places_highest_priority_first() {
        let small = solid(8, 8, red());
        let big = solid(16, 16, green());
        let inputs = [
            AtlasInput { name: "low", image: &small, priority: 5 },
            AtlasInput { name: "high", image: &big, priority: 1 },
        ];
        let params = AtlasParams { max_size: 32, padding: 0, bleed: 0, allow_rotation: false, sidecar: SidecarFormat::Json, trim_sheet: false, priority_arrange: true };
        let r = pack(&inputs, &params).unwrap();
        let high = r.tiles.iter().find(|t| t.name == "high").unwrap();
        let low = r.tiles.iter().find(|t| t.name == "low").unwrap();
        assert_eq!((high.x, high.y), (0, 0), "highest priority placed at origin");
        assert_eq!((low.x, low.y), (16, 0), "lower priority placed after");
    }
    #[test]
    fn sidecar_json_has_expected_shape() {
        let a = solid(8, 8, red());
        let inputs = [AtlasInput { name: "a.png", image: &a, priority: 3 }];
        let r = pack(&inputs, &AtlasParams::default()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&sidecar_json(&r)).unwrap();
        assert_eq!(v["format"], "texelbox-atlas/1");
        assert_eq!(v["atlas"]["width"], r.size);
        assert_eq!(v["tileCount"], 1);
        let t = &v["tiles"][0];
        assert_eq!(t["name"], "a.png");
        assert_eq!(t["width"], 8);
        assert!((t["uv"]["u1"].as_f64().unwrap() - (2.0 + 8.0) / 16.0).abs() < 1e-5);
        let xml = sidecar_xml(&r);
        assert!(xml.contains("<atlas format=\"texelbox-atlas/1\""));
        assert!(xml.contains("name=\"a.png\""));
    }
    #[test]
    fn empty_input_is_rejected() {
        assert!(matches!(pack(&[], &AtlasParams::default()), Err(AtlasError::Empty)));
    }
}
