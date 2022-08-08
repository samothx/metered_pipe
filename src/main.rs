extern crate core;

use anyhow::{anyhow, Context, Result};
use std::env::args;

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

struct F64Consts {
    max_byte_f64: f64,
    max_kb_f64: f64,
    max_mb_f64: f64,
    kb_size_f64: f64,
    mb_size_f64: f64,
    gb_size_f64: f64,
}

impl F64Consts {
    fn new() -> F64Consts {
        F64Consts {
            max_byte_f64: f64::from(TWO_KB as u32),
            max_kb_f64: f64::from(TWO_MB as u32),
            max_mb_f64: f64::from(TWO_GB as u32),
            kb_size_f64: f64::from(KB_SIZE as u32),
            mb_size_f64: f64::from(MB_SIZE as u32),
            gb_size_f64: f64::from(GB_SIZE as u32),
        }
    }
}

const MIN_BUF_SIZE: usize = 256;
const MAX_BUF_SIZE: usize = MB_SIZE * 32;

fn print_help() {
    eprintln!("metered_pipe - pipe stdin to stdout while printing amount of data and throughput to stderr");
    eprintln!("USAGE: metered_pipe [-s] [-h]");
    eprintln!("  -s: use static instead of adaptive buffer");
    eprintln!("  -t: report totals only");
    eprintln!("  -h: print this help text");
}

fn format_flow(bytes: usize, seconds: f64, f64_consts: &F64Consts) -> String {
    assert!(seconds >= 1.0);

    let flow = if bytes <= TWO_GB {
        f64::from(bytes as u32) / seconds
    } else if bytes <= HALF_TB {
        f64::from((bytes / MB_SIZE) as u32) / seconds * f64_consts.mb_size_f64
    } else {
        f64::from((bytes / GB_SIZE) as u32) / seconds * f64_consts.gb_size_f64
    };

    if flow <= f64_consts.max_byte_f64 {
        format!("{:.2} Bytes/sec", flow)
    } else if flow <= f64_consts.max_kb_f64 {
        format!("{:.2} KB/sec", flow / f64_consts.kb_size_f64)
    } else if flow <= f64_consts.max_mb_f64 {
        format!("{:.2} MB/sec", flow / f64_consts.mb_size_f64)
    } else {
        format!("{:.2} GB/sec", flow / f64_consts.gb_size_f64)
    }
}

fn format_bytes(bytes: usize, f64_consts: &F64Consts) -> String {
    match bytes {
        0..=MAX_BYTE => format!("{} Bytes", bytes),
        MAX_BYTE_NEXT..=MAX_KB => {
            format!("{:.2} KB", f64::from(bytes as u32) / f64_consts.kb_size_f64)
        }
        MAX_KB_NEXT..=MAX_MB => {
            format!("{:.2} MB", f64::from(bytes as u32) / f64_consts.mb_size_f64)
        }
        MAX_MB_NEXT..=HALF_TB => format!(
            "{:.2} GB",
            f64::from((bytes / MB_SIZE) as u32) / f64_consts.kb_size_f64
        ),
        _ => format!("{} GB", bytes / GB_SIZE),
    }
}

fn main() -> Result<()> {
    let f64_consts = F64Consts::new();
    let mut adaptive_buffer = true;
    let mut totals_only = false;
    let mut buf_size = MB_SIZE;

    for arg in args().skip(1) {
        match arg.as_str() {
            "-s" => {
                adaptive_buffer = true;
                buf_size = KB_SIZE;
            }
            "-t" => {
                totals_only = true;
            }
            _ => {
                print_help();
                return Err(anyhow!(
                    "metered_pipe: invalid command line argument {}",
                    arg
                ));
            }
        };
    }

    if totals_only {
        adaptive_buffer = false;
        buf_size = MB_SIZE;
    }

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
                        format_bytes(bytes_written, &f64_consts),
                        elapsed,
                        format_flow(bytes_written, elapsed, &f64_consts),
                    );
                    return Ok(());
                } else {
                    stdout_handle
                        .write(&buf[..size])
                        .with_context(|| "failed to write to stdout")?;
                    bytes_written += size;

                    if !totals_only {
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
                                format_bytes(bytes_written, &f64_consts),
                                format_flow(bytes_written, elapsed, &f64_consts)
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
            }
            Err(e) => return Err(anyhow!("failed to read from stdin: {:?}", e)),
        }
    }
}
