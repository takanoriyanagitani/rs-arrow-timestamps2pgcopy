use std::io;
use std::process::ExitCode;
use std::sync::Arc;

use io::Write;

use arrow_schema::DataType;
use arrow_schema::Field;
use arrow_schema::Schema;

use arrow_schema::TimeUnit;

use arrow_array::RecordBatch;

use arrow_array::ArrayRef;

use arrow_array::Int64Array;

use arrow_array::TimestampMicrosecondArray;
use arrow_array::TimestampMillisecondArray;
use arrow_array::TimestampSecondArray;

use rs_arrow_timestamps2pgcopy::ref2unixtime;
use rs_arrow_timestamps2pgcopy::ref2unixtime_ms;
use rs_arrow_timestamps2pgcopy::ref2unixtime_us;

use rs_arrow_timestamps2pgcopy::TimestampLike;

fn io_record_batch() -> impl FnMut() -> Result<RecordBatch, io::Error> {
    || {
        RecordBatch::try_new(
            Schema::new(vec![
                Field::new("id", DataType::Int64, false),
                Field::new(
                    "timestamp",
                    DataType::Timestamp(TimeUnit::Second, None),
                    false,
                ),
                Field::new(
                    "timestamp_ms",
                    DataType::Timestamp(TimeUnit::Millisecond, None),
                    false,
                ),
                Field::new(
                    "timestamp_us",
                    DataType::Timestamp(TimeUnit::Microsecond, None),
                    false,
                ),
            ])
            .into(),
            vec![
                Arc::new(Int64Array::from(vec![2, 3, 5, 7])),
                Arc::new(TimestampSecondArray::from(vec![0, 1, 2, 3])),
                Arc::new(TimestampMillisecondArray::from(vec![0, 1, 2, 3])),
                Arc::new(TimestampMicrosecondArray::from(vec![0, 1, 2, 3])),
            ],
        )
        .map_err(io::Error::other)
    }
}

#[derive(Debug, PartialEq)]
enum TimestampArray<'a> {
    Ignore,
    Seconds(&'a TimestampSecondArray),
    Millis(&'a TimestampMillisecondArray),
    Micros(&'a TimestampMicrosecondArray),
}

impl<'a> TimestampArray<'a> {
    fn to_writer<W>(&self, trusted_index: usize, wtr: &mut W) -> Result<(), io::Error>
    where
        W: Write,
    {
        match self {
            Self::Seconds(l) => l.to_writer(trusted_index, wtr),
            Self::Millis(l) => l.to_writer(trusted_index, wtr),
            Self::Micros(l) => l.to_writer(trusted_index, wtr),
            _ => Ok(()),
        }
    }
}

fn io_bat2stdout(rbat: RecordBatch) -> impl FnOnce() -> Result<(), io::Error> {
    move || {
        let rowcnt: usize = rbat.num_rows();
        let cols: &[ArrayRef] = rbat.columns();

        let mapd: Vec<TimestampArray> = cols
            .iter()
            .map(|aref| {
                if let Some(i) = ref2unixtime(aref) {
                    return TimestampArray::Seconds(i);
                }

                if let Some(i) = ref2unixtime_ms(aref) {
                    return TimestampArray::Millis(i);
                }

                if let Some(i) = ref2unixtime_us(aref) {
                    return TimestampArray::Micros(i);
                }

                TimestampArray::Ignore
            })
            .filter(|i| !i.eq(&TimestampArray::Ignore))
            .collect();

        let o = io::stdout();
        let mut ol = o.lock();

        let mut colsize: [u8; 2] = [0; 2];
        colsize[1] = mapd.len() as u8;

        for i in 0..rowcnt {
            ol.write_all(&colsize)?;
            for col in mapd.iter() {
                col.to_writer(i, &mut ol)?;
            }
        }
        Ok(())
    }
}

const PGCOPY_SIGN: &[u8] = b"PGCOPY\n\xff\r\n\0";
const PGCOPY_FLAGS: [u8; 4] = [0; 4];
const PGCOPY_EXT: [u8; 4] = [0; 4];

const PGCOPY_TRAILER: [u8; 2] = u16::to_be_bytes(0xffff);

fn io_header2stdout() -> impl FnOnce() -> Result<(), io::Error> {
    || {
        let o = io::stdout();
        let mut ol = o.lock();
        let mut buf: [u8; 19] = [0; 19];
        let src = PGCOPY_SIGN.iter().chain(&PGCOPY_FLAGS).chain(&PGCOPY_EXT);
        let z = buf.iter_mut().zip(src);
        for pair in z {
            let (dst, src) = pair;
            *dst = *src;
        }
        ol.write_all(&buf)?;
        ol.flush()
    }
}

fn io_trailer2stdout() -> impl FnOnce() -> Result<(), io::Error> {
    || {
        let o = io::stdout();
        let mut ol = o.lock();
        ol.write_all(&PGCOPY_TRAILER)?;
        ol.flush()
    }
}

fn io_main() -> impl FnOnce() -> Result<(), io::Error> {
    || {
        io_header2stdout()()?;
        let rbat: RecordBatch = io_record_batch()()?;
        io_bat2stdout(rbat)()?;
        io_trailer2stdout()()?;
        Ok(())
    }
}

fn sub() -> Result<(), io::Error> {
    io_main()()
}

fn main() -> ExitCode {
    sub().map(|_| ExitCode::SUCCESS).unwrap_or_else(|e| {
        eprintln!("{e}");
        ExitCode::FAILURE
    })
}
