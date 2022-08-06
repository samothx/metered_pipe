extern crate core;

use anyhow::{anyhow, Context, Result};
use std::env::args;

use lazy_static::lazy_static;
use std::io::{stdin, stdout, Read, Write};
use std::time::Instant;

const KB_SIZE: usize = 1024;
const MB_SIZE: usize = KB_SIZE * KB_SIZE;
const GB_SIZE: usize = MB_SIZE * KB_SIZE;

const TWO_KB: usize = 2 * KB_SIZE;
const TWO_MB: usize = 2 * MB_SIZE;
const TWO_GB: usize = 2 * GB_SIZE;
const HALF_TB: usize = GB_SIZE * 512;

const MAX_BYTE: usize = TWO_KB;
const MAX_BYTE_NEXT: usize = MAX_BYTE + 1;

const MAX_KB: usize = TWO_MB;
const MAX_KB_NEXT: usize = MAX_KB + 1;

const MAX_MB: usize = TWO_GB; // cannot be more than 2 GB - sorry, this is where the 32bit overflow happens
const MAX_MB_NEXT: usize = MAX_MB + 1;

lazy_static! {
    static ref MAX_BYTE_F64: f64 = f64::from(TWO_KB as u32);
    static ref MAX_KB_F64: f64 = f64::from(TWO_MB as u32);
    static ref MAX_MB_F64: f64 = f64::from(TWO_GB as u32);
    static ref KB_SIZE_F64: f64 = f64::from(KB_SIZE as u32);
    static ref MB_SIZE_F64: f64 = f64::from(MB_SIZE as u32);
    static ref GB_SIZE_F64: f64 = f64::from(GB_SIZE as u32);
}

const MIN_BUF_SIZE: usize = 256;
const MAX_BUF_SIZE: usize = MB_SIZE * 32;

fn print_help() {
    eprintln!("metered_pipe - pipe stdin to stdout while printing amount of data and throughput to stderr");
    eprintln!("USAGE: metered_pipe [-s] [-h]");
    eprintln!("  -s: use static instead of adaptive buffer");
    eprintln!("  -h: print this help text");
}

fn format_flow(bytes: usize, seconds: f64) -> String {
    assert!(seconds >= 1.0);

    let flow = if bytes <= TWO_GB {
        f64::from(bytes as u32) / seconds
    } else if bytes <= HALF_TB {
        f64::from((bytes / MB_SIZE) as u32) / seconds * *MB_SIZE_F64
    } else {
        f64::from((bytes / GB_SIZE) as u32) / seconds * *GB_SIZE_F64
    };

    if flow <= *MAX_BYTE_F64 {
        format!("{:.2} Bytes/sec", flow)
    } else if flow <= *MAX_KB_F64 {
        format!("{:.2} KB/sec", flow / *KB_SIZE_F64)
    } else if flow <= *MAX_MB_F64 {
        format!("{:.2} MB/sec", flow / *MB_SIZE_F64)
    } else {
        format!("{:.2} GB/sec", flow / *GB_SIZE_F64)
    }
}

fn format_bytes(bytes: usize) -> String {
    match bytes {
        0..=MAX_BYTE => format!("{} Bytes", bytes),
        MAX_BYTE_NEXT..=MAX_KB => format!("{:.2} KB", f64::from(bytes as u32) / *KB_SIZE_F64),
        MAX_KB_NEXT..=MAX_MB => format!("{:.2} MB", f64::from(bytes as u32) / *MB_SIZE_F64),
        MAX_MB_NEXT..=HALF_TB => format!(
            "{:.2} GB",
            f64::from((bytes / MB_SIZE) as u32) / *KB_SIZE_F64
        ),
        _ => format!("{} GB", bytes / GB_SIZE),
    }
}

fn main() -> Result<()> {
    let (adaptive_buffer, mut buf_size) = if let Some(arg) = args().nth(1) {
        if arg == "-s" {
            (false, MB_SIZE)
        } else if arg == "-h" {
            print_help();
            return Ok(());
        } else {
            print_help();
            return Err(anyhow!(
                "metered_pipe: invalid command line argument {}",
                arg
            ));
        }
    } else {
        (true, KB_SIZE)
    };

    let data_in = stdin();
    let mut stdin_handle = data_in.lock();
    let data_out = stdout();
    let mut stdout_handle = data_out.lock();
    let mut bytes_written = 0usize;
    let mut last_print = 0f64;
    let mut buf: Vec<u8> = vec![0; buf_size];
    let start_time = Instant::now();

    loop {
        match stdin_handle.read(&mut buf) {
            Ok(size) => {
                if size == 0 {
                    let elapsed = (Instant::now() - start_time).as_secs_f64();
                    eprintln!(
                        "{} in {:?}, {}",
                        format_bytes(bytes_written),
                        elapsed,
                        format_flow(bytes_written, elapsed),
                    );
                    return Ok(());
                } else {
                    stdout_handle
                        .write(&buf[..size])
                        .with_context(|| "failed to write to stdout")?;
                    bytes_written += size;

                    let elapsed = (Instant::now() - start_time).as_secs_f64();
                    if elapsed - last_print >= 1.0 {
                        if adaptive_buffer
                            && (size == buf_size)
                            && (buf_size >= MIN_BUF_SIZE * 2)
                            && (elapsed - last_print > 2.0)
                        {
                            buf_size /= 2;
                            buf.resize(buf_size, 0);
                        }

                        last_print = elapsed;
                        eprint!(
                            "{}, {}   \u{d}",
                            format_bytes(bytes_written),
                            format_flow(bytes_written, elapsed)
                        )
                    } else if adaptive_buffer
                        && (buf_size <= MAX_BUF_SIZE / 2)
                        && (elapsed - last_print < 0.25)
                    {
                        buf_size *= 2;
                        buf.resize(buf_size, 0);
                    }
                }
            }
            Err(e) => return Err(anyhow!("failed to read from stdin: {:?}", e)),
        }
    }
}
