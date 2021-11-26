use crate::msf;
use crate::util;
use std::io::{BufReader, Read};

mod hashtable;
/// Result type alias for this module
type Result<T> = std::result::Result<T, Error>;
type SerializedHashTable<T> = hashtable::SerializedHashTable<T>;

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
    HashTableError,
    /// key not found in the StreamMap.
    StreamMapKeyNotFound(String),
}

impl From<hashtable::Error> for Error{
    fn from(error: hashtable::Error) -> Self{
        Error::HashTableError
    }
}
impl From<std::io::Error> for Error{
    fn from(error: std::io::Error) -> Self{
        Error::Consume(error)
    }
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
pub struct NamedStreamMap {
    str_len: u32,
    strings: Vec<String>,
    hash_table: SerializedHashTable<u32>,
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
        return Ok(PDBStreamHeader {
            version: util::consume!(reader, u32, "version")?,
            signature: util::consume!(reader, u32, "signature")?,
            age: util::consume!(reader, u32, "age")?,
            unique_id: util::consume!(reader, u128, "unique_id")?,
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
        let a = util::consume!(msfsr, 0x2000, "Test");
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
        println!("{:?}", strm_map.get_stream_number(String::from("/LinkInfo")));
        Ok(ret)
    }
}
