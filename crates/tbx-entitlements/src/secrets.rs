const XOR_KEY: [u8; 32] = [
    0x89, 0xD5, 0x04, 0xD3, 0x42, 0x66, 0xCA, 0x1E, 0xF2, 0xAA, 0xFB, 0x5F, 0xB6, 0xE1, 0xBE, 0xC4, 0x57, 0x0B, 0x0D, 0xFD, 0xA7, 0x27, 0xA8, 0x72, 0xA6, 0x68, 0x35, 0x20, 0x77, 0xC5, 0x79, 0xA3,
];
pub const fn xor_const<const N: usize>(src: &[u8]) -> [u8; N] {
    let mut out = [0u8; N];
    let mut i = 0;
    while i < N {
        out[i] = src[i] ^ XOR_KEY[i % XOR_KEY.len()];
        i += 1;
    }
    out
}
pub fn restore<const N: usize>(obf: &[u8; N]) -> String {
    let bytes: Vec<u8> = obf.iter().enumerate().map(|(i, b)| b ^ XOR_KEY[i % XOR_KEY.len()]).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}
#[macro_export]
macro_rules! obf_str {
    ($s:expr) => {{
        const INPUT: &str = $s;
        const OBFD: [u8; INPUT.as_bytes().len()] =
            $crate::secrets::xor_const::<{ INPUT.as_bytes().len() }>(INPUT.as_bytes());
        $crate::secrets::restore::<{ INPUT.as_bytes().len() }>(&OBFD)
    }};
}
const SERVER_URL_PLAIN: &str = "https://texelbox-license.imadedar98.workers.dev";
pub fn server_url() -> String {
    const OBFD: [u8; SERVER_URL_PLAIN.len()] =
        xor_const::<{ SERVER_URL_PLAIN.len() }>(SERVER_URL_PLAIN.as_bytes());
    restore::<{ SERVER_URL_PLAIN.len() }>(&OBFD)
}
pub const PRODUCTION_VERIFY_KEY: [u8; 32] = [
    0x89, 0xD5, 0x04, 0xD3, 0x42, 0x66, 0xCA, 0x1E, 0xF2, 0xAA, 0xFB, 0x5F, 0xB6, 0xE1, 0xBE, 0xC4,
    0x57, 0x0B, 0x0D, 0xFD, 0xA7, 0x27, 0xA8, 0x72, 0xA6, 0x68, 0x35, 0x20, 0x77, 0xC5, 0x79, 0xA3,
];
pub const DEV_SIGNING_KEY: [u8; 32] = [
    0x7E, 0x58, 0xC1, 0x3B, 0x94, 0x02, 0x6D, 0xF5, 0x1A, 0xB7, 0x4E, 0x83, 0xD9, 0x60, 0x2F, 0xAA,
    0x0B, 0x71, 0xE6, 0x5C, 0x98, 0x34, 0xAD, 0x17, 0xF0, 0x69, 0x25, 0xCC, 0x48, 0xB3, 0x8E, 0x05,
];
pub fn has_production_key() -> bool {
    PRODUCTION_VERIFY_KEY.iter().any(|b| *b != 0)
}
pub fn device_id() -> String {
    use std::sync::OnceLock;
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        let dir = directories::ProjectDirs::from("app", "TexelBox", "TexelBox");
        if let Some(dir) = dir {
            let path = dir.config_dir().join("device-id");
            if let Ok(existing) = std::fs::read_to_string(&path) {
                let trimmed = existing.trim().to_string();
                if !trimmed.is_empty() {
                    return trimmed;
                }
            }
            let _ = std::fs::create_dir_all(dir.config_dir());
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let id = format!("dev-{:x}-{:x}", now, std::process::id() as u64 * 2654435761 % 0xFFFF_FFFF);
            let _ = std::fs::write(&path, &id);
            return id;
        }
        "dev-ephemeral".to_string()
    })
    .clone()
}
