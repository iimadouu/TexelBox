use crate::maps::MapOutputs;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueLevel {
    Info,
    Warning,
    Error,
}
#[derive(Clone, Debug)]
pub struct Issue {
    pub level: IssueLevel,
    pub key: &'static str,
    pub args: Vec<(&'static str, String)>,
}
impl Issue {
    pub fn new(level: IssueLevel, key: &'static str) -> Self {
        Self { level, key, args: Vec::new() }
    }
    pub fn arg(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.args.push((key, value.into()));
        self
    }
}
pub fn validate_maps(outputs: &MapOutputs) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut dims: Vec<(&str, u32, u32)> = Vec::new();
    if let Some(ref h) = outputs.height {
        dims.push(("height", h.width, h.height));
    }
    if let Some(ref n) = outputs.normal {
        dims.push(("normal", n.width(), n.height()));
    }
    if let Some(ref r) = outputs.roughness {
        dims.push(("roughness", r.width, r.height));
    }
    if let Some(ref a) = outputs.ao {
        dims.push(("ao", a.width, a.height));
    }
    if dims.len() > 1 {
        for i in 1..dims.len() {
            if dims[i].1 != dims[0].1 || dims[i].2 != dims[0].2 {
                issues.push(
                    Issue::new(IssueLevel::Error, "preview-issue-mismatch")
                        .arg("a", dims[0].0)
                        .arg("b", dims[i].0),
                );
            }
        }
    }
    for &(name, w, h) in &dims {
        if !w.is_power_of_two() || !h.is_power_of_two() {
            issues.push(
                Issue::new(IssueLevel::Warning, "preview-issue-not-pot")
                    .arg("map", name)
                    .arg("w", w.to_string())
                    .arg("h", h.to_string()),
            );
        }
    }
    if let Some(ref n) = outputs.normal {
        check_normal_map(n, &mut issues);
    }
    issues
}
fn check_normal_map(normal: &image::RgbaImage, issues: &mut Vec<Issue>) {
    let (w, h) = normal.dimensions();
    if w == 0 || h == 0 {
        return;
    }
    let mut sum = [0.0f64; 3];
    let mut sum_sq = [0.0f64; 3];
    let npix = (w as u64 * h as u64) as f64;
    for pixel in normal.pixels() {
        for c in 0..3 {
            let v = pixel[c] as f64;
            sum[c] += v;
            sum_sq[c] += v * v;
        }
    }
    let mean: Vec<f64> = sum.iter().map(|s| s / npix).collect();
    let variance: f64 = (0..3)
        .map(|c| ((sum_sq[c] / npix) - mean[c] * mean[c]).max(0.0))
        .sum();
    let stddev = variance.sqrt();
    if stddev < 2.0 && mean[2] < 200.0 {
        issues.push(Issue::new(IssueLevel::Warning, "preview-issue-flat-normal"));
    }
    if mean[2] < 80.0 && (100.0..160.0).contains(&mean[0]) && (100.0..160.0).contains(&mean[1]) {
        issues.push(Issue::new(IssueLevel::Warning, "preview-issue-normal-blue-low"));
    }
    if mean[0] < 60.0 && mean[1] < 60.0 && mean[2] > 180.0 {
        issues.push(Issue::new(IssueLevel::Info, "preview-issue-looks-like-height"));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrayF32;
    use image::RgbaImage;
    fn flat_normal(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_pixel(w, h, image::Rgba([128, 128, 255, 255]))
    }
    fn noisy_normal(w: u32, h: u32) -> RgbaImage {
        let mut img = RgbaImage::new(w, h);
        for (x, y, p) in img.enumerate_pixels_mut() {
            let nx = ((x % 256) as i32 - 128) as f32 / 128.0;
            let ny = ((y % 256) as i32 - 128) as f32 / 128.0;
            *p = image::Rgba([
                ((nx * 0.5 + 0.5) * 255.0) as u8,
                ((ny * 0.5 + 0.5) * 255.0) as u8,
                255,
                255,
            ]);
        }
        img
    }
    #[test]
    fn flat_normal_does_not_warn_for_valid_up_facing() {
        let issues = validate_maps(&MapOutputs {
            height: None,
            normal: Some(flat_normal(256, 256)),
            roughness: None,
            ao: None,
        });
        assert!(
            issues.iter().all(|i| i.key != "preview-issue-flat-normal"),
            "flat-up normal should not produce a flat-normal warning"
        );
    }
    #[test]
    fn corrupt_constant_gray_normal_warns() {
        let corrupt = RgbaImage::from_pixel(64, 64, image::Rgba([128, 128, 128, 255]));
        let issues = validate_maps(&MapOutputs {
            height: None,
            normal: Some(corrupt),
            roughness: None,
            ao: None,
        });
        assert!(
            issues.iter().any(|i| i.key == "preview-issue-flat-normal"),
            "constant-gray normal map should trigger flat-normal warning"
        );
    }
    #[test]
    fn noisy_normal_has_no_contrast_warnings() {
        let issues = validate_maps(&MapOutputs {
            height: None,
            normal: Some(noisy_normal(256, 256)),
            roughness: None,
            ao: None,
        });
        assert!(issues.iter().all(|i| i.key != "preview-issue-flat-normal"));
    }
    #[test]
    fn mismatch_detected() {
        let issues = validate_maps(&MapOutputs {
            height: Some(GrayF32::new(64, 64)),
            normal: Some(RgbaImage::new(128, 128)),
            roughness: None,
            ao: None,
        });
        assert!(issues.iter().any(|i| i.key == "preview-issue-mismatch"));
    }
    #[test]
    fn matching_set_has_no_mismatch() {
        let issues = validate_maps(&MapOutputs {
            height: Some(GrayF32::new(128, 128)),
            normal: Some(noisy_normal(128, 128)),
            roughness: Some(GrayF32::new(128, 128)),
            ao: None,
        });
        assert!(issues.iter().all(|i| i.key != "preview-issue-mismatch"));
    }
    #[test]
    fn non_pot_warns() {
        let issues = validate_maps(&MapOutputs {
            height: Some(GrayF32::new(100, 100)),
            normal: Some(RgbaImage::new(100, 100)),
            roughness: None,
            ao: None,
        });
        assert!(issues.iter().any(|i| i.key == "preview-issue-not-pot"));
    }
}
