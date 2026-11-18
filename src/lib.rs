use std::io;

use io::Write;

use arrow_array::Array;
use arrow_array::ArrayRef;
use arrow_array::cast::AsArray;

use arrow_array::TimestampMicrosecondArray;
use arrow_array::TimestampMillisecondArray;
use arrow_array::TimestampSecondArray;

pub const NULL_BYTES: [u8; 4] = u32::to_be_bytes(0xffff_ffff);

pub trait TimestampLike: Array {
    fn to_qword(&self, trusted_index: usize) -> [u8; 8];

    fn to_writer<W>(&self, trusted_index: usize, wtr: &mut W) -> Result<(), io::Error>
    where
        W: Write,
    {
        if self.is_null(trusted_index) {
            wtr.write_all(&NULL_BYTES)?;
            return Ok(());
        }

        let bytes: [u8; 8] = self.to_qword(trusted_index);

        // Buffer layout:
        // 3 bytes padding/header,
        // 1 byte for length (8),
        // 8 bytes for value
        // Total = 12 bytes
        let mut buf: [u8; 12] = [0; 12];
        buf[3] = 8;
        buf[4..12].copy_from_slice(&bytes);

        wtr.write_all(&buf)
    }
}

/// The offset to convert the pg epoch based timestamp to unixtime in us.
pub const PG_EPOCH_OFFSET: i64 = 10957 * 86400 * 1000 * 1000;

impl TimestampLike for &TimestampMicrosecondArray {
    fn to_qword(&self, trusted_index: usize) -> [u8; 8] {
        let unixtime_us: i64 = self.value(trusted_index);
        let pg_timestamp: i64 = unixtime_us.saturating_sub(PG_EPOCH_OFFSET);
        pg_timestamp.to_be_bytes()
    }
}

impl TimestampLike for &TimestampMillisecondArray {
    fn to_qword(&self, trusted_index: usize) -> [u8; 8] {
        let unixtime_ms: i64 = self.value(trusted_index);
        let unixtime_us: i64 = unixtime_ms.saturating_mul(1000);
        let pg_timestamp: i64 = unixtime_us.saturating_sub(PG_EPOCH_OFFSET);
        pg_timestamp.to_be_bytes()
    }
}

impl TimestampLike for &TimestampSecondArray {
    fn to_qword(&self, trusted_index: usize) -> [u8; 8] {
        let unixtime: i64 = self.value(trusted_index);
        let unixtime_ms: i64 = unixtime.saturating_mul(1000);
        let unixtime_us: i64 = unixtime_ms.saturating_mul(1000);
        let pg_timestamp: i64 = unixtime_us.saturating_sub(PG_EPOCH_OFFSET);
        pg_timestamp.to_be_bytes()
    }
}

pub fn ref2unixtime(r: &ArrayRef) -> Option<&TimestampSecondArray> {
    r.as_primitive_opt()
}

pub fn ref2unixtime_ms(r: &ArrayRef) -> Option<&TimestampMillisecondArray> {
    r.as_primitive_opt()
}

pub fn ref2unixtime_us(r: &ArrayRef) -> Option<&TimestampMicrosecondArray> {
    r.as_primitive_opt()
}
