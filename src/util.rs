const BASE62_MAP: &[u8] =
    "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz".as_bytes();

pub fn fnv1a32(data: &[u8]) -> u32 {
    let mut hash: u32 = 2166136261;
    for byte in data {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash
}

pub fn base62(n: u32) -> Vec<u8> {
    let mut num = n;
    let mut ret: Vec<u8> = Vec::new();
    loop {
        let i = num % 62;
        ret.insert(0, BASE62_MAP[i as usize]);
        num = num / 62;
        if num <= 0 {
            break;
        }
    }
    ret
}

pub fn hash(data: &str) -> String {
    let n = fnv1a32(data.as_bytes());
    let s = base62(n);
    String::from_utf8_lossy(&s).to_string()
}
