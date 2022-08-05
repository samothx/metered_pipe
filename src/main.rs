extern crate core;

use anyhow::{anyhow, Context, Result};

use lazy_static::lazy_static;
use std::io::{stdin, stdout, Read, Write};
use std::time::Instant;

const KB_SIZE: usize = 1024;
const MB_SIZE: usize = KB_SIZE * KB_SIZE;
const GB_SIZE: usize = MB_SIZE * KB_SIZE;

const MAX_BYTE: usize = KB_SIZE * 2;
const MAX_BYTE_NEXT: usize = MAX_BYTE + 1;

const MAX_KB: usize = MB_SIZE * 2;
const MAX_KB_NEXT: usize = MAX_KB + 1;

const MAX_MB: usize = GB_SIZE * 2; // cannot be more sorry, this is where the 32bit overflow happens

lazy_static! {
    static ref KB_SIZE_F64: f64 = f64::from(KB_SIZE as u32);
    static ref MB_SIZE_F64: f64 = f64::from(MB_SIZE as u32);
    static ref GB_SIZE_F64: f64 = f64::from(GB_SIZE as u32);
}

const BUF_SIZE: usize = MB_SIZE; // 1Mb

fn format_bytes(bytes: usize) -> String {
    match bytes {
        0..=MAX_BYTE => format!("{} Bytes", bytes),
        MAX_BYTE_NEXT..=MAX_KB => format!("{:.2} KB", f64::from(bytes as u32) / *KB_SIZE_F64),
        MAX_KB_NEXT..=MAX_MB => format!("{:.2} MB", f64::from(bytes as u32) / *MB_SIZE_F64),
        _ => format!(
            "{:.2} GB",
            f64::from((bytes / MB_SIZE) as u32) / *KB_SIZE_F64
        ),
    }
}

fn main() -> Result<()> {
    let stdin_stream = stdin();
    let mut stdin_handle = stdin_stream.lock();
    let mut buf: Vec<u8> = vec![0; BUF_SIZE];
    let stdout_stream = stdout();
    let mut stdout_handle = stdout_stream.lock();
    let mut bytes_written: usize = 0;
    let start = Instant::now();
    let mut last_print = 0u64;
    loop {
        match stdin_handle.read(&mut buf) {
            Ok(size) => {
                if size == 0 {
                    let elapsed = Instant::now() - start;
                    eprintln!(
                        "{} in {:?}, {}/sec",
                        format_bytes(bytes_written),
                        elapsed,
                        format_bytes(bytes_written / elapsed.as_secs() as usize),
                    );
                    return Ok(());
                } else {
                    bytes_written += size;
                    stdout_handle
                        .write(&buf[..size])
                        .with_context(|| "failed to write to stdout")?;

                    let elapsed = (Instant::now() - start).as_secs();
                    if elapsed > last_print {
                        last_print = elapsed;

                        eprint!(
                            "{}, {}/sec    {}",
                            format_bytes(bytes_written),
                            format_bytes(bytes_written / elapsed as usize),
                            '\u{d}'
                        )
                    }
                }
            }
            Err(e) => return Err(anyhow!("failed to read from stdin: {:?}", e)),
        }
    }
}
