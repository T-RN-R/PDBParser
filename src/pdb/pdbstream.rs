use crate::msf;
use crate::pdb::hashtable::SerializedHashTable;
use crate::util;
use std::io::Read;

type Result<T> = std::result::Result<T, Error>;
#[derive(Debug)]
/// All of the errors that could possible be returned from this module
pub enum Error {
    /// Something bad happened, and we have no clue what it was.
    Unknown,
    /// Error consuming from the underlying  reader.
    Consume(std::io::Error),
    /// There was an issue with the stream
    BadStream(u32, msf::Error),
    /// The version number was invalid
    InvalidVersion,
    /// The read HashTable was invalid
    HashTableError(crate::pdb::hashtable::Error),
    /// key not found in the StreamMap.
    StreamMapKeyNotFound(String),
    /// Feature code unrecognized
    UnkownFeatureCode(u32)
}
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Consume(error)
    }
}
impl From<crate::pdb::hashtable::Error> for Error {
    fn from(error: crate::pdb::hashtable::Error) -> Self {
        Error::HashTableError(error)
    }
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
pub struct NamedStreamMap {
    str_len: u32,
    strings: Vec<String>,
    hash_table: SerializedHashTable<u32>,
}

#[derive(Debug, Default)]
pub struct PdbStream {
    hdr: PDBStreamHeader,
    stream_map: NamedStreamMap,
   // feature_codes : PDBFeatureCodeList
}
#[derive(std::cmp::PartialEq,Debug)]
enum PDBFeatureCode {
    VC110 = 20091201,
    VC140 = 20140508,
    NoTypeMerge = 0x4D544F4E,
    MinimalDebugInfo = 0x494E494D,
}
#[derive(Debug, Default)]
struct PDBFeatureCodeList {
    codes: Vec<PDBFeatureCode>,
}
impl PDBFeatureCodeList {
    pub fn load(reader: &mut msf::MSFStreamReader<std::fs::File>) -> Result<Self> {
        let ret: Vec<PDBFeatureCode> = Vec::with_capacity(1);
        let val = util::consume!(reader, u32, "test")?;

        if PDBFeatureCode::VC110 as u32 != val
            && PDBFeatureCode::VC140 as u32 != val
            && PDBFeatureCode::MinimalDebugInfo as u32 != val
            && PDBFeatureCode::NoTypeMerge as u32 != val
        {return Err(Error::UnkownFeatureCode(val));}
        Ok(PDBFeatureCodeList { codes: ret })
    }
}
impl NamedStreamMap {
    pub fn load(reader: &mut msf::MSFStreamReader<std::fs::File>) -> Result<Self> {
        let mut strm_map = NamedStreamMap::default();
        strm_map.str_len = util::consume!(reader, u32, "str_len")?;
        let mut b = vec![0_u8; strm_map.str_len as usize];
        let bytes = reader
            .read_exact(&mut b)
            .map(|_| b)
            .map_err(|x| Error::Consume(x))
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
        let ret = PDBStreamHeader {
            version: util::consume!(reader, u32, "version")?,
            signature: util::consume!(reader, u32, "signature")?,
            age: util::consume!(reader, u32, "age")?,
            unique_id: util::consume!(reader, u128, "unique_id")?,
        };
        if !ret.check_version(PDBStreamVersion::VC70) {
            return Err(Error::InvalidVersion);
        }
        return Ok(ret);
    }
    pub fn check_version(&self, other_ver: PDBStreamVersion) -> bool {
        self.version == other_ver as u32
    }
}

impl PdbStream {
    pub fn load(reader: &mut msf::MSFStreamReader<std::fs::File>) -> Result<Self> {
        return Ok(PdbStream {
            hdr: PDBStreamHeader::load(reader)?,
            stream_map: NamedStreamMap::load(reader)?,
           // feature_codes : PDBFeatureCodeList::load(reader)?
        });
    }
    pub fn get_stream_number(&self, name: String) -> Result<u32> {
        self.stream_map.get_stream_number(name)
    }
}
