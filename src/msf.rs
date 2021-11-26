    use crate::util;
    use std::io::{BufReader, Read, Seek, SeekFrom};
    type Result<T> = std::result::Result<T, Error>;

    #[derive(Debug)]
    pub enum Error {
        Unknown,
        Consume(std::io::Error),
        NotPDBFile,
        InvalidBlockSize(u32),
        Seek(std::io::Error),
        StreamDirectoryTooSmall,
        StreamNumberOutOfBounds,
        BlockNumberOutOfBounds,
    }
    impl From<std::io::Error> for Error{
        fn from(error: std::io::Error) -> Self{
            Error::Consume(error)
        }
    }
    #[derive(Default)]
    pub struct MSF {
        sb: SuperBlock,
        sd: StreamDirectory,
    }

    #[derive(Default)]
    struct SuperBlock {
        /// Must be equal to "Microsoft C / C++ MSF 7.00\\r\\n" followed by the bytes 1A 44 53 00 00 00.
        file_magic: [u8; 0x20],
        /// The block size of the internal file system. Valid values are 512, 1024, 2048, and 4096 bytes.
        block_size: u32,
        /// FreeBlockMapBlock can only be 1 or 2!
        free_block_map: u32,
        /// The total number of blocks in the file. NumBlocks * BlockSize should equal the size of the file on disk.
        num_blocks: u32,
        ///The size of the stream directory, in bytes.
        num_directory_bytes: u32,
        /// ?
        unknown: u32,
        /// The index of a block within the MSF file.
        block_map_addr: u32,
    }
    #[derive(Default)]
    struct StreamDirectory {
        num_streams: u32,
        stream_sizes: Vec<u32>,       // stream_sizes[num_streams]
        stream_blocks: Vec<Vec<u32>>, // stream_blocks[num_streams][ceil(stream_sizes/block_size)]
    }
    impl StreamDirectory {
        pub fn load(reader: &mut (impl Read + Seek), sb: &SuperBlock) -> Result<Self> {
            /// Reads through the StreamDirectory to build a list of streams
            
            let mut ret = Self::default();
            // Need to find the StreamDirectory!
            // Using BlockMapAddr, we find a block indexed into the file where the stream dir indexes reside.
            //
            // saturating divide
            let num_indirection_entries =
                (sb.num_directory_bytes + sb.block_size - 1) / sb.block_size;
            let stream_dir_indirection_offset = (sb.block_map_addr * sb.block_size) as u64;
            reader
                .seek(SeekFrom::Start(stream_dir_indirection_offset))
                .map_err(Error::Seek)?;
            // The StreamDirectory could be stored across multiple blocks, so there is a layer of indirection here
            //  which is an array of u32 indicating the underlying blocks of the StreamDirectory. This array is located
            //  at the `block_map_addr`th block in the MSF, with ceil(num_directory_bytes / block_size) entries.
            let mut indirection_blocks: Vec<u32> =
                Vec::with_capacity(num_indirection_entries as usize);
            for _ in 0..num_indirection_entries {
                indirection_blocks.push(util::consume!(reader, u32, "Stream Directory Fragment Blocks")?)
            }
            println!("StreamDirectory blocks{:?}", indirection_blocks);
            let mut cur_indirection_block = 0;
            let first_block = *indirection_blocks
                .get(cur_indirection_block)
                .ok_or(Error::StreamDirectoryTooSmall)?;
            cur_indirection_block += 1;
            reader
                .seek(SeekFrom::Start((first_block * sb.block_size) as u64))
                .map_err(Error::Seek)?;
            let mut bytes_to_read = sb.block_size;
            ret.num_streams = util::consume!(reader, u32, "Number of Streams")?;
            bytes_to_read -= 4;

            let streams_left: u32 = ret.num_streams;
            // now read streams_left u32s from the blocks.
            for cur_stream in 0..streams_left {
                ret.stream_sizes.push(util::consume!(reader, u32, "Stream Size")?);
                bytes_to_read -= 4;
                if bytes_to_read == 0 {
                    let next_blk = *indirection_blocks
                        .get(cur_indirection_block)
                        .ok_or(Error::StreamDirectoryTooSmall)?;
                    cur_indirection_block += 1;
                    reader
                        .seek(SeekFrom::Start((next_blk * sb.block_size) as u64))
                        .map_err(Error::Seek)?;
                    bytes_to_read = sb.block_size;
                    println!("Reading from block: {:?}", next_blk);
                }
            }
            println!("stream_sizes : {:?}", ret.stream_sizes);

            if bytes_to_read == 0 {
                //check to ensure we don't need to move to the next block.
                let next_blk = *indirection_blocks
                    .get(cur_indirection_block)
                    .ok_or(Error::StreamDirectoryTooSmall)?;
                cur_indirection_block += 1;
                reader
                    .seek(SeekFrom::Start((next_blk * sb.block_size) as u64))
                    .map_err(Error::Seek)?;
                bytes_to_read = sb.block_size;
                println!("Reading from block: {:?}", next_blk);
            }
            //stream_blocks: Vec<Vec<u32>>, // stream_blocks[num_streams][ceil(stream_sizes/block_size)]
            //Now we have to get a hold of the stream_blocks :/
            for stream_size in &ret.stream_sizes {
                let num_blocks_in_stream = (stream_size + sb.block_size - 1) / sb.block_size;
                let mut cur_vec: Vec<u32> = Vec::with_capacity(num_blocks_in_stream as usize);
                //now read num_blocks_in_stream entries!
                for block_num in 0..num_blocks_in_stream {
                    cur_vec.push(util::consume!(reader, u32, "Block_Size")?);
                    bytes_to_read -= 4;
                    if bytes_to_read == 0 {
                        let next_blk = *indirection_blocks
                            .get(cur_indirection_block)
                            .ok_or(Error::StreamDirectoryTooSmall)?;
                        cur_indirection_block += 1;
                        reader
                            .seek(SeekFrom::Start((next_blk * sb.block_size) as u64))
                            .map_err(Error::Seek)?;
                        bytes_to_read = sb.block_size;
                        println!("Reading from block: {:?}", next_blk);
                    }
                }
                ret.stream_blocks.push(cur_vec);
            }
            println!("stream_blocks : {:?}", ret.stream_blocks);

            Ok(ret)
        }
    }
    impl SuperBlock {
        pub fn load(reader: &mut (impl Read + Seek)) -> Result<Self> {
            let mut ret = Self::default();
            ret.file_magic = util::consume!(reader, 0x20, "MSF Header")?;
            if &ret.file_magic != b"Microsoft C/C++ MSF 7.00\r\n\x1aDS\x00\x00\x00" {
                return Err(Error::NotPDBFile);
            }
            ret.block_size = util::consume!(reader, u32, "Block Size")?;
            if ret.block_size != 512u32
                && ret.block_size != 1024u32
                && ret.block_size != 2048u32
                && ret.block_size != 4096u32
            {
                return Err(Error::InvalidBlockSize(ret.block_size));
            }
            ret.free_block_map = util::consume!(reader, u32, "Free Block Map")?;
            ret.num_blocks = util::consume!(reader, u32, "Num Blocks")?;
            ret.num_directory_bytes = util::consume!(reader, u32, "Num Directory Bytes")?;
            ret.unknown = util::consume!(reader, u32, "Unknown")?;
            ret.block_map_addr = util::consume!(reader, u32, "Block Map Addr")?;
            Ok(ret)
        }
    }

    impl MSF {
        pub fn load(reader: &mut (impl Read + Seek)) -> Result<Self> {
            let mut ret = Self::default();
            println!("Hello????");
            ret.sb = SuperBlock::load(reader)?;
            println!("block_size : {:?}", ret.sb.block_size);
            println!("free_block_map : {:?}", ret.sb.free_block_map);
            println!("num_blocks : {:?}", ret.sb.num_blocks);
            println!("num_directory_bytes : {:?}", ret.sb.num_directory_bytes);
            println!("unknown : {:?}", ret.sb.unknown);
            println!("block_map_addr : {:?}", ret.sb.block_map_addr);
            println!("File Size : {:?}", ret.sb.num_blocks * ret.sb.block_size);

            ret.sd = StreamDirectory::load(reader, &ret.sb)?;
            //get the number of streams in the StreamDirectory

            // Now that we have the header, we want to skip ahead by free_block_map * block_size
            //reader.seek(SeekFrom::Current((ret.sb.block_size * ret.sb.free_block_map) as i64))
            //.map_err(Error::Seek)?;
            Ok(ret)
        }
        pub fn block_size(self: Self) -> usize {
            self.sb.block_size as usize
        }
    }

    pub struct MSFStreamReader<'a, T> {
        reader: &'a mut BufReader<T>,
        msf: &'a mut MSF,
        current_block_idx: u32,
        current_bytes_into_block: u32,
        stream_number: u32,
    }
    impl<'a> MSFStreamReader<'a, std::fs::File> {
        pub fn new(
            buf_reader: &'a mut BufReader<std::fs::File>,
            msf_struct: &'a mut MSF,
            strm_num: u32,
        ) -> std::result::Result<Self, Error> {
            if msf_struct.sd.num_streams <= strm_num {
                return Err(Error::StreamNumberOutOfBounds);
            }
            let mut ret = MSFStreamReader {
                reader: buf_reader,
                msf: msf_struct,
                current_block_idx: 0,
                current_bytes_into_block: 0,
                stream_number: strm_num,
            };
            return Ok(ret);
        }
        pub fn change_stream(&mut self, stream_no: u32) -> Result<()> {
            if self.msf.sd.num_streams <= stream_no {
                return Err(Error::StreamNumberOutOfBounds);
            }
            self.stream_number = stream_no;
            Ok(())
        }
    }
    impl<'a> Read for MSFStreamReader<'a, std::fs::File> {
        fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
            let mut amt_read: usize = 0;
            let mut amt_to_read = buf.len();
            let blocks = self
                .msf
                .sd
                .stream_blocks
                .get(self.stream_number as usize)
                .ok_or(Error::StreamNumberOutOfBounds)
                .expect("What?");
            let amt_available_to_read = (blocks.len() - self.current_block_idx as usize)
                * self.msf.sb.block_size as usize
                + self.current_bytes_into_block as usize;

            // Need to respect current seek position, so check to ensure we don't go past the end of the stream
            if amt_available_to_read < amt_to_read as usize {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "Could not read more bytes than there are in the buffer!",
                ));
            }
            while amt_to_read != 0 {
                let amt_until_block_end: usize =
                    (self.msf.sb.block_size - self.current_bytes_into_block) as usize;
                if amt_until_block_end > amt_to_read {
                    //Handle the case where we don't have to advance to the next block
                    //Need to seek the underlying BufReader.
                    let cur_block = blocks
                        .get(self.current_block_idx as usize)
                        .ok_or(Error::BlockNumberOutOfBounds)
                        .expect("What?");
                    let seek_pos =
                        (cur_block * self.msf.sb.block_size) + self.current_bytes_into_block;
                    self.reader
                        .seek(SeekFrom::Start(seek_pos as u64))
                        .map_err(Error::Seek)
                        .expect("Seek failed");
                    //println!("StreamPos {:?} ", self.reader.stream_position());

                    let mut tmp = vec![0u8; amt_to_read];
                    //println!("Reading {:?} u8s", amt_to_read);
                    let data = self
                        .reader
                        .read_exact(&mut tmp)
                        .map(|_| tmp)
                        .map_err(|x| Error::Consume(x))
                        .expect("What?");
                    //println!(
                    //   "Read block :{:?} \ndata: {:?}",
                    //    blocks.get(self.current_block_idx as usize),
                    //    data
                    // );
                    buf[amt_read..(amt_read + amt_to_read)].copy_from_slice(&data);
                    self.current_bytes_into_block += amt_to_read as u32;
                    amt_read += amt_to_read as usize;
                    amt_to_read -= amt_to_read;
                } else {
                    //Handle the case where we read to the end of the current block
                    //Need to seek the underlying BufReader
                    let cur_block = blocks
                        .get(self.current_block_idx as usize)
                        .ok_or(Error::BlockNumberOutOfBounds)
                        .expect("What?");
                    let seek_pos =
                        (cur_block * self.msf.sb.block_size) + self.current_bytes_into_block;
                    self.reader
                        .seek(SeekFrom::Start(seek_pos as u64))
                        .map_err(Error::Seek)
                        .expect("Seek failed");
                    //println!("StreamPos {:?} ", self.reader.stream_position());

                    let mut tmp = vec![0u8; amt_until_block_end];
                    //println!("Reading {:?} u8s", amt_until_block_end);

                    let data = self
                        .reader
                        .read_exact(&mut tmp)
                        .map(|_| tmp)
                        .map_err(|x| Error::Consume(x))
                        .expect("What?");
                    //println!(
                    //    "Read block :{:?} \ndata: {:?}",
                    //   blocks.get(self.current_block_idx as usize),
                    //    data
                    //);

                    buf[amt_read..(amt_read + amt_until_block_end)].copy_from_slice(&data);
                    amt_to_read -= amt_until_block_end;
                    amt_read += amt_until_block_end;
                    self.current_block_idx += 1;
                    self.current_bytes_into_block = 0;
                }
            }
            Ok(amt_read)
        }
    }
    impl<'a> Seek for MSFStreamReader<'a, std::fs::File> {
        fn seek(&mut self, pos: SeekFrom) -> std::result::Result<u64, std::io::Error> {
            /// Seek to pos bytes into the stream
            println!("Seeking from: {:?}", pos);
            let mut p = 0;
            let blocks = self
                .msf
                .sd
                .stream_blocks
                .get(self.stream_number as usize)
                .ok_or(Error::StreamNumberOutOfBounds)
                .expect("What?");

            let max_pos = (blocks).len() * (self.msf.sb.block_size as usize);
            println!("Max ps : {:?}", max_pos);

            match pos {
                SeekFrom::Start(position) => {
                    println!("Seeking from: {:?}", position);

                    if position > max_pos as u64 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "Supplied position out of bounds!",
                        ));
                    }
                    // Floor division here.
                    let block_entry = position / self.msf.sb.block_size as u64;
                    // Get proper pos in block
                    let pos_in_block = position - (block_entry * self.msf.sb.block_size as u64);
                    self.current_block_idx = block_entry as u32;
                    let cur_block = *blocks
                        .get(block_entry as usize)
                        .ok_or(Error::BlockNumberOutOfBounds)
                        .expect("Something went wrong");
                    if pos_in_block > self.msf.sb.block_size as u64 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "Current position does not match block size",
                        ));
                    }
                    self.current_bytes_into_block = pos_in_block as u32;
                    println!(
                        "Current block, entry: {:?}:{:?} , Current bytes into block: {:?}",
                        cur_block, block_entry, pos_in_block
                    );
                }
                //SeekFrom::End(position) => {},
                //SeekFrom::Current(position) => {},
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "Unsupported enum type for SeekFrom::",
                    ));
                }
            }
            Ok(p)
        }
    }
