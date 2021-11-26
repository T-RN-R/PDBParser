
macro_rules! consume {
    ($reader:expr, $ty:ty, $field:expr) => {{
        let mut tmp = [0_u8; std::mem::size_of::<$ty>()];
        $reader
            .read_exact(&mut tmp)
            .map(|_| <$ty>::from_le_bytes(tmp))
            .map_err(|x| std::io::Error::new(std::io::ErrorKind::Other, "Consumption Error"))
    }};
    ($reader:expr, $size:expr, $field:expr) => {{
        let mut tmp = [0_u8; $size];
        $reader
            .read_exact(&mut tmp)
            .map(|_| tmp)
            .map_err(|x| std::io::Error::new(std::io::ErrorKind::Other, "Consumption Error"))
    }};
}

pub(crate) use consume;