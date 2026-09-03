use ed25519_dalek::{Signature, SigningKey, Verifier};
pub const INTEGRITY_MESSAGE: &[u8] = b"texelbox-integrity-v1";
const DEV_KNOWN_VECTOR: [u8; 64] = [
    0x6B, 0xF0, 0xE6, 0x4D, 0xD6, 0x55, 0xD7, 0x21, 0x50, 0x98, 0x80, 0x62, 0x84, 0xDA, 0xB3, 0x2D,
    0x71, 0x65, 0x76, 0x9E, 0x51, 0x62, 0xDA, 0x41, 0xD3, 0xC0, 0xF4, 0x52, 0xF3, 0xE8, 0xA4, 0xF4,
    0x87, 0xE9, 0x4B, 0xBB, 0xB3, 0xDC, 0x36, 0x57, 0x67, 0xF2, 0x06, 0x50, 0x07, 0xAC, 0xD2, 0xDA,
    0x80, 0x94, 0x0A, 0xBE, 0x93, 0x7F, 0x5A, 0xDC, 0xCC, 0xD4, 0x4E, 0x0C, 0xBE, 0x3E, 0x0A, 0x0C,
];
pub fn debugger_attached() -> bool {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn IsDebuggerPresent() -> i32;
            fn CheckRemoteDebuggerPresent(h_process: isize, debugger_present: *mut i32) -> i32;
            fn GetCurrentProcess() -> isize;
        }
        unsafe {
            if IsDebuggerPresent() != 0 {
                return true;
            }
            let mut present = 0i32;
            if CheckRemoteDebuggerPresent(GetCurrentProcess(), &mut present) != 0 && present != 0 {
                return true;
            }
        }
        false
    }
    #[cfg(not(windows))]
    {
        false
    }
}
pub fn key_ring_intact() -> bool {
    check_vector(&crate::secrets::DEV_SIGNING_KEY, &DEV_KNOWN_VECTOR)
}
fn check_vector(key_bytes: &[u8; 32], expected_sig: &[u8; 64]) -> bool {
    let signing = SigningKey::from_bytes(key_bytes);
    let verifying = signing.verifying_key();
    let sig = Signature::from_bytes(expected_sig);
    verifying.verify(INTEGRITY_MESSAGE, &sig).is_ok()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets;
    use ed25519_dalek::Signer;
    #[test]
    fn print_known_vector() {
        let sk = SigningKey::from_bytes(&secrets::DEV_SIGNING_KEY);
        let sig = sk.sign(INTEGRITY_MESSAGE);
        let bytes = sig.to_bytes();
        let rows: Vec<String> = bytes
            .chunks(16)
            .map(|row| {
                row.iter()
                    .map(|b| format!("0x{b:02X}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .collect();
        println!("DEV_KNOWN_VECTOR rows:\n{}", rows.join(",\n"));
    }
    #[test]
    fn vector_matches_embedded_key() {
        assert!(key_ring_intact(), "embedded integrity vector does not verify — regenerate via print_known_vector");
    }
    #[test]
    fn tampered_key_fails_integrity() {
        let mut tampered = secrets::DEV_SIGNING_KEY;
        tampered[0] ^= 0xFF;
        assert!(!check_vector(&tampered, &DEV_KNOWN_VECTOR));
    }
}
