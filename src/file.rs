use super::error::Error;
use std::io::SeekFrom;
use std::path::Path;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader, BufWriter};

const NULL: u8 = b'\0';
const CR: u8 = b'\r';
const LF: u8 = b'\n';

pub async fn open_create(path: &Path) -> Result<File, Error> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await?;
    Ok(file)
}

pub async fn open_read(path: &Path) -> Result<File, Error> {
    let file = OpenOptions::new().read(true).open(&path).await?;
    Ok(file)
}

pub async fn read(
    reader: &mut BufReader<File>,
    buf: &mut [u8],
    reader_pos: u64,
    mode: &str,
    lastch: Option<u8>,
) -> Result<(usize, usize, Option<u8>), Error> {
    let offset = SeekFrom::Start(reader_pos);
    reader.seek(offset).await?;

    let ret = if mode == "octet" {
        read_octet(reader, lastch, buf).await?
    } else {
        read_netascii(reader, lastch, buf).await?
    };

    Ok(ret)
}

#[cfg(target_family = "windows")]
async fn read_netascii(
    reader: &mut BufReader<File>,
    lastch: Option<u8>,
    buf: &mut [u8],
) -> Result<(usize, usize, Option<u8>), Error> {
    let mut index = 0;
    let mut reader_pos = 0;
    let mut lastch = lastch;

    while index < buf.len() {
        let ch = match reader.read_u8().await {
            Ok(ch) => ch,
            _ => break,
        };
        reader_pos += 1;

        if ch != LF {
            if let Some(ch) = lastch {
                // CR -> CR NULL
                buf[index] = ch;
                index += 1;
                lastch = None;

                if buf.len() <= index {
                    reader_pos -= 1;
                    break;
                }
            }
        }

        buf[index] = ch;
        index += 1;
        lastch = if ch == CR { Some(NULL) } else { None };
    }

    Ok((reader_pos, index, lastch))
}

#[cfg(target_family = "unix")]
async fn read_netascii(
    reader: &mut BufReader<File>,
    lastch: Option<u8>,
    buf: &mut [u8],
) -> Result<(usize, usize, Option<u8>), Error> {
    let mut index = 0;
    let mut reader_pos = 0;
    let mut lastch = lastch;

    while index < buf.len() {
        if let Some(ch) = lastch {
            // CR -> CR NULL
            // LF -> CR LF
            buf[index] = ch;
            index += 1;
            lastch = None;

            if buf.len() <= index {
                break;
            }
        }

        let ch = match reader.read_u8().await {
            Ok(ch) => ch,
            _ => break,
        };
        reader_pos += 1;

        if ch == LF {
            // LF -> CR LF
            buf[index] = CR;
            index += 1;
            lastch = Some(ch);

            if buf.len() <= index {
                break;
            }
        }

        buf[index] = ch;
        index += 1;
        lastch = if ch == CR { Some(NULL) } else { None };
    }

    Ok((reader_pos, index, lastch))
}

async fn read_octet(
    reader: &mut BufReader<File>,
    _: Option<u8>,
    buf: &mut [u8],
) -> Result<(usize, usize, Option<u8>), Error> {
    let size = reader.read(buf).await?;
    Ok((size, size, None))
}

pub async fn write(
    writer: &mut BufWriter<File>,
    buf: &[u8],
    mode: &str,
    lastch: Option<u8>,
) -> Result<(usize, Option<u8>), Error> {
    let offset = SeekFrom::End(0);
    writer.seek(offset).await?;

    let ret = if mode == "octet" {
        write_octet(writer, lastch, buf).await?
    } else {
        write_netascii(writer, lastch, buf).await?
    };

    writer.flush().await?;

    Ok(ret)
}

async fn write_netascii(
    writer: &mut BufWriter<File>,
    lastch: Option<u8>,
    buf: &[u8],
) -> Result<(usize, Option<u8>), Error> {
    let mut size = 0;
    let mut lastch = lastch;

    for &ch in buf {
        match ch {
            NULL if lastch.is_some() => {
                // CR NULL -> CR
                lastch = None;
                continue;
            }
            CR => {
                lastch = Some(ch);
            }
            LF if lastch.is_some() => {
                // CR LF -> LF
                if !cfg!(windows) {
                    let pre_pos = SeekFrom::Current(-1);
                    writer.seek(pre_pos).await?;
                }

                lastch = None;
            }
            _ => {
                lastch = None;
            }
        }

        writer.write_u8(ch).await?;
        size += 1;
    }

    Ok((size, lastch))
}

async fn write_octet(
    writer: &mut BufWriter<File>,
    _: Option<u8>,
    buf: &[u8],
) -> Result<(usize, Option<u8>), Error> {
    let size = writer.write(buf).await?;
    Ok((size, None))
}
