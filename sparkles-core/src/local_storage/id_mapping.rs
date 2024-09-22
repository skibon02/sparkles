//! Simple hash map, aimed for better insertion performance.
//! Memory overhead: 5*elements_cnt
//!
//! 256 id variants => 1280 bytes
//!
//! get overhead ~1ns

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum EventType {
    Instant,
    RangeStart,
    RangeEnd(u8),
}

impl EventType {
    fn get_offs(&self) -> u32 {
        match self {
            Self::Instant => 0,
            Self::RangeStart => 1,
            Self::RangeEnd(_) => 2,
        }
    }
}

#[derive(Clone)]
struct U32U8Map {
    keys: [Option<u32>; 256],
    values: [Option<u8>; 256],
}

impl Default for U32U8Map {
    fn default() -> Self {
        Self {
            keys: [None; 256],
            values: [None; 256],
        }
    }
}

impl U32U8Map {
    const fn new() -> Self {
        Self {
            keys: [None; 256],
            values: [None; 256],
        }
    }

    fn hash(&self, key: u32) -> usize {
        (core::num::Wrapping(key).0 as usize).wrapping_mul(2654435761) % 256
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

#[derive(Clone, Default)]
pub struct IdStoreRepr {
    /// Used internally for faster lookup
    id_map: U32U8Map,
    last_id: u8,

    tags_store: IdStoreMap,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct IdStoreMap {
    pub tags: Vec<(String, EventType)>,
}
impl IdStoreMap {
    pub const fn new() -> Self {
        Self {
            tags: Vec::new()
        }
    }
}
impl From<IdStoreRepr> for IdStoreMap {
    fn from(id_store: IdStoreRepr) -> Self {
        id_store.tags_store
    }
}

impl IdStoreRepr {
    pub const fn new() -> Self {
        Self {
            id_map: U32U8Map::new(),
            last_id: 0,
            tags_store: IdStoreMap::new(),
        }
    }

    #[inline(always)]
    pub fn insert_and_get_id(&mut self, hash: u32, tag: &str, event_type: EventType) -> u8 {
        let offs = event_type.get_offs();
        let hash = hash + offs;
        match self.id_map.get(hash) {
            Some(v) => {
                v
            },
            None => {
                let last_id = self.last_id;
                self.last_id += 1;
                self.id_map.insert(hash, last_id).unwrap();
                self.tags_store.tags.push((tag.to_string(), event_type));
                last_id
            }
        }
    }
}