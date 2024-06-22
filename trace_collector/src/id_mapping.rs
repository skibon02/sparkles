//! Overhead: 5*elements_cnt
//!
//! 256 id variants => 1280 bytes
//!
//! get overhead ~10ns
struct U32U8Map {
    keys: [Option<u32>; 256],
    values: [Option<u8>; 256],
}

impl U32U8Map {
    const fn new() -> Self {
        Self {
            keys: [None; 256],
            values: [None; 256],
        }
    }

    fn hash(&self, key: u32) -> usize {
        (std::num::Wrapping(key).0 as usize).wrapping_mul(2654435761) % 256
    }

    fn insert(&mut self, key: u32, value: u8) -> Result<(), &'static str> {
        let mut idx = self.hash(key);
        for _ in 0..256 {
            if self.keys[idx].is_none() || self.keys[idx] == Some(key) {
                self.keys[idx] = Some(key);
                self.values[idx] = Some(value);
                return Ok(());
            }
            idx = (idx + 1) % 256;
        }
        Err("Map is full")
    }

    fn get(&self, key: u32) -> Option<u8> {
        let mut idx = self.hash(key);
        for _ in 0..256 {
            if let Some(existing_key) = self.keys[idx] {
                if existing_key == key {
                    return self.values[idx];
                }
            } else {
                return None;
            }
            idx = (idx as u8).wrapping_add(1) as usize;
        }
        None
    }
}

pub struct IdStore {
    id_map: U32U8Map,
    last_id: u8,
}

impl IdStore {
    pub const fn new() -> Self {
        Self {
            id_map: U32U8Map::new(),
            last_id: 0,
        }
    }

    /// Intended way to use: map.get_id_or_insert(id_map!("tag_name");
    pub fn insert_and_get_id(&mut self, hash: u32, tag: &str) -> u8 {
        match self.id_map.get(hash) {
            Some(v) => {
                v
            },
            None => {
                self.last_id += 1;
                self.id_map.insert(hash, self.last_id).unwrap();
                self.last_id
            }
        }
    }
}