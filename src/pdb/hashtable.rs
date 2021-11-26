
use crate::msf;
use crate::util;
use std::io::{BufReader, Read};

type Result<T> = std::result::Result<T, Error>;
#[derive(Debug)]
/// All of the errors that could possible be returned from this module
pub enum Error {
    Unknown,
    Consume(std::io::Error),
    HashTableInvalid,
    HashTableEntryNotFound(u32)
}
impl From<std::io::Error> for Error{
    fn from(error: std::io::Error) -> Self{
        Error::Consume(error)
    }
}

#[derive(Debug, Default)]
struct HashTableEntry<T> {
    key: u32,
    value: T,
}
#[derive(Debug, Default)]
struct BitVector {
    word_count: u32,
    words: Vec<u8>,
}
#[derive(Debug, Default)]
pub struct SerializedHashTable<T> {
    size: usize,
    capacity: usize,
    present_vec: BitVector,
    deleted_vec: BitVector,
    entries: Vec<HashTableEntry<T>>,
}

impl HashTableEntry<u32> {
    pub fn load(reader: &mut msf::MSFStreamReader<std::fs::File>) -> Result<Self> {
        return Ok(HashTableEntry {
            key: util::consume!(reader, u32, "k")?,
            value: util::consume!(reader, u32, "v")?,
        });
    }
}
impl BitVector {
    pub fn load(reader: &mut msf::MSFStreamReader<std::fs::File>) -> Result<Self> {
        let wc = util::consume!(reader, u32, "word_count")?;

        let mut b = vec![0_u8; (wc * 4) as usize];
        for i in 0..wc {
            b[i as usize] = util::consume!(reader, u8, "BitVec")?;
        }

        let ret = BitVector {
            word_count: wc,
            words: b,
        };
        return Ok(ret);
    }
}

impl SerializedHashTable<u32> {
    pub fn load(reader: &mut msf::MSFStreamReader<std::fs::File>) -> Result<Self> {
        let mut ret = Self::default();
        ret.size = util::consume!(reader, u32, "size")? as usize;
        ret.capacity = util::consume!(reader, u32, "capacity")? as usize;
        ret.present_vec = BitVector::load(reader)?;
        ret.deleted_vec = BitVector::load(reader)?;
        ret.entries = Vec::with_capacity(ret.capacity);
        for _ in 0..ret.capacity {
            ret.entries.push(HashTableEntry::load(reader)?);
        }
        return Ok(ret);
    }

    pub fn get(&self, key: u32) -> Result<u32> {
        //First, need to isolate the "present" entries, build a list of valid entries
        let mut indices: Vec<u32> = Vec::with_capacity(self.present_vec.word_count as usize); // assume we only use a quarter of the bitvector, for perf
        for (i, bv) in self.present_vec.words.iter().enumerate() {
            //bv is an u8;
            for n in 0..8 {
                let val = bv >> n & 1;
                if val == 1 {
                    indices.push(n as u32 + (i * 8) as u32);
                }
            }
        }

        for idx in indices {
            let entry = self
                .entries
                .get(idx as usize)
                .ok_or(Error::HashTableInvalid)?;
            if entry.key == key {
                return Ok(entry.value);
            }
        }
        return Err(Error::HashTableEntryNotFound(key));
    }
}