use crate::msf;
use std::io::{BufReader, Read};
use std::mem::size_of;
/// Result type alias for this module
type Result<T> = std::result::Result<T, Error>;

/// Consumes from a Read impl
macro_rules! consume {
    ($reader:expr, $ty:ty, $field:expr) => {{
        let mut tmp = [0_u8; size_of::<$ty>()];
        $reader
            .read_exact(&mut tmp)
            .map(|_| <$ty>::from_le_bytes(tmp))
            .map_err(|x| Error::Consume($field, x))
    }};
    ($reader:expr, $size:expr, $field:expr) => {{
        let mut tmp = [0_u8; $size];
        $reader
            .read_exact(&mut tmp)
            .map(|_| tmp)
            .map_err(|x| Error::Consume($field, x))
    }};
}
#[derive(Debug)]
/// All of the errors that could possible be returned from this module
pub enum Error {
    /// Something bad happened, and we have no clue what it was.
    Unknown,
    /// Error consuming from the underlying  reader.
    Consume(&'static str, std::io::Error),
    /// There was an issue with the stream
    BadStream(u32, msf::Error),
    /// The version number was invalid
    InvalidVersion,
    /// The read HashTable was invalid
    HashTableInvalid,
    /// Lookup entry in hash table was not found
    HashTableEntryNotFound(u32),
    /// key not found in the StreamMap.
    StreamMapKeyNotFound(String),
}
#[derive(Default)]
pub struct PDB {
    pdb_strm_hdr: PDBStreamHeader,
}

pub enum PDBStreamVersion {
    VC2 = 19941610,
    VC4 = 19950623,
    VC41 = 19950814,
    VC50 = 19960307,
    VC98 = 19970604,
    VC70Dep = 19990604,
    VC70 = 20000404,
    VC80 = 20030901,
    VC110 = 20091201,
    VC140 = 20140508,
}
#[derive(Debug, Default)]
pub struct PDBStreamHeader {
    version: u32,
    signature: u32,
    age: u32,
    unique_id: u128,
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
#[derive(Debug, Default)]
pub struct NamedStreamMap {
    str_len: u32,
    strings: Vec<String>,
    hash_table: SerializedHashTable<u32>,
}

impl HashTableEntry<u32> {
    pub fn load(reader: &mut msf::MSFStreamReader<std::fs::File>) -> Result<Self> {
        return Ok(HashTableEntry {
            key: consume!(reader, u32, "k")?,
            value: consume!(reader, u32, "v")?,
        });
    }
}
impl BitVector {
    pub fn load(reader: &mut msf::MSFStreamReader<std::fs::File>) -> Result<Self> {
        let wc = consume!(reader, u32, "word_count")?;

        let mut b = vec![0_u8; (wc * 4) as usize];
        for i in 0..wc {
            b[i as usize] = consume!(reader, u8, "BitVec")?;
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
        ret.size = consume!(reader, u32, "size")? as usize;
        ret.capacity = consume!(reader, u32, "capacity")? as usize;
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
impl NamedStreamMap {
    pub fn load(reader: &mut msf::MSFStreamReader<std::fs::File>) -> Result<Self> {
        let mut strm_map = NamedStreamMap::default();
        strm_map.str_len = consume!(reader, u32, "str_len")?;
        let mut b = vec![0_u8; strm_map.str_len as usize];
        let bytes = reader
            .read_exact(&mut b)
            .map(|_| b)
            .map_err(|x| Error::Consume("Read", x))
            .expect("What?");
        strm_map.strings = Vec::with_capacity(1);
        let mut last_str_pos = 0;
        for (i, b) in bytes.iter().enumerate() {
            if *b == 0_u8 {
                strm_map.strings.push(
                    std::str::from_utf8(&bytes[last_str_pos..i])
                        .unwrap()
                        .to_string(),
                );
                last_str_pos = i + 1;
            }
        }
        println!("Found the following streams: {:?}", strm_map.strings);
        strm_map.hash_table = SerializedHashTable::load(reader)?;
        println!("{:?}", strm_map.hash_table);
        return Ok(strm_map);
    }

    pub fn get_stream_number(&self, name: String) -> Result<u32> {
        let pos = self
            .strings
            .iter()
            .position(|r| *r == name)
            .ok_or(Error::StreamMapKeyNotFound(name))?;
        return Ok(self.hash_table.get(pos as u32)?);
    }
}
impl PDBStreamHeader {
    pub fn load(reader: &mut msf::MSFStreamReader<std::fs::File>) -> Result<Self> {
        return Ok(PDBStreamHeader {
            version: consume!(reader, u32, "version")?,
            signature: consume!(reader, u32, "signature")?,
            age: consume!(reader, u32, "age")?,
            unique_id: consume!(reader, u128, "unique_id")?,
        });
    }
}

impl PDB {
    pub fn old_directory(
        reader: &mut BufReader<std::fs::File>,
        msf: &mut msf::MSF,
    ) -> Result<Self> {
        let mut ret = Self::default();
        let mut msfsr =
            msf::MSFStreamReader::new(reader, msf, 0).map_err(|x| Error::BadStream(0, x))?;

        //msfsr.seek(SeekFrom::Start(8191)).map_err(|x|{
        //    PdbError::Seek(x)
        //})?;
        let a = consume!(msfsr, 0x2000, "Test");
        println!("{:?}", a);
        Ok(ret)
    }
    pub fn pdb_stream(reader: &mut BufReader<std::fs::File>, msf: &mut msf::MSF) -> Result<Self> {
        let mut ret = Self::default();
        let mut msfsr =
            msf::MSFStreamReader::new(reader, msf, 1).map_err(|x| Error::BadStream(0, x))?;

        //msfsr.seek(SeekFrom::Start(8191)).map_err(|x|{
        //    PdbError::Seek(x)
        //})?;
        let pdb_strm_hdr: PDBStreamHeader = PDBStreamHeader::load(&mut msfsr).map_err(|x| x)?;
        if pdb_strm_hdr.version != PDBStreamVersion::VC70 as u32 {
            return Err(Error::InvalidVersion);
        }
        ret.pdb_strm_hdr = pdb_strm_hdr;
        let strm_map = NamedStreamMap::load(&mut msfsr)?;
        println!("{:?}", strm_map.get_stream_number(String::from("/names")));
        Ok(ret)
    }
}
