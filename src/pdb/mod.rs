
mod hashtable;
mod pdbstream;

use crate::msf;
use crate::util;
use pdbstream::PdbStream;
use std::io::{BufReader, Read};

/// Result type alias for this module
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
    HashTableError(hashtable::Error),
    /// key not found in the StreamMap.
    StreamMapKeyNotFound(String),
    /// Error parsing Stream
    PdbStreamError(pdbstream::Error)
}

impl From<hashtable::Error> for Error{
    fn from(error: hashtable::Error) -> Self{
        Error::HashTableError(error)
    }
}
impl From<std::io::Error> for Error{
    fn from(error: std::io::Error) -> Self{
        Error::Consume(error)
    }
}
impl From<pdbstream::Error> for Error{
    fn from(error: pdbstream::Error) -> Self{
        Error::PdbStreamError(error)
    }
}


#[derive(Default)]
pub struct PDB {
    pdb_strm: PdbStream,
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
        let pdb_stream = PdbStream::load(&mut msfsr)?;
        println!("{:?}", pdb_stream.get_stream_number(String::from("/LinkInfo")));
        Ok(ret)
    }
}
