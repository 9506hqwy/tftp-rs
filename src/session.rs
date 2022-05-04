use super::error::Error;
use super::file;
use super::options::Options;
use super::packet;
use super::{HEADER_LEN, ROLLOVER};
use bytes::Bytes;
use log::{trace, warn};
use std::future::Future;
use std::net::SocketAddr;
use tokio::fs::File;
use tokio::io::{BufReader, BufWriter};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};

pub struct TftpSession {
    blocknum_ack: u16,
    blocknum_blocks: Vec<FileBlock>,
    received_data: u16,
    sock: UdpSocket,
    remote_addr: SocketAddr,
    local_file: Option<TftpSessionFile>,
    mode: String,
    options: Options,
    rollover: u32,
    lastch: Option<u8>,
}

pub enum TftpSessionFile {
    Reader(Mutex<BufReader<File>>),
    Writer(BufWriter<File>),
}

struct FileBlock {
    blocknum: u16,
    reader_pos: u64,
    lastch: Option<u8>,
    data_len: usize,
    reader_pos_len: usize,
}

impl TftpSession {
    pub fn new(sock: UdpSocket, remote_addr: SocketAddr) -> Self {
        TftpSession {
            blocknum_ack: 0,
            blocknum_blocks: vec![],
            received_data: 0,
            sock,
            remote_addr,
            local_file: None,
            mode: "netascii".to_string(),
            options: Options::default(),
            rollover: 0,
            lastch: None,
        }
    }

    pub fn remote_addr(&self) -> &SocketAddr {
        &self.remote_addr
    }

    pub fn blocknum_ack(&self) -> u16 {
        self.blocknum_ack
    }

    pub fn set_blocknum_ack(&mut self, num: u16) {
        self.blocknum_ack = num;
    }

    pub fn blocknum_ack_add(&self, value: u16) -> u16 {
        match self.blocknum_ack.checked_add(value) {
            Some(v) => v,
            _ => ROLLOVER,
        }
    }

    pub fn blocknum_expect(&self, num: u16) -> bool {
        let min = self.blocknum_ack_add(1);
        let max = self.blocknum_ack_add(self.options().windowsize());
        if min <= max {
            min <= num && num <= max
        } else {
            min <= num || num <= max
        }
    }

    pub fn received_data_clear(&mut self) {
        self.received_data = 0;
    }

    pub fn received_data_last(&self) -> bool {
        self.received_data == self.options().windowsize()
    }

    pub fn received_data_inc(&mut self) {
        self.received_data += 1;
    }

    pub fn sent_completed(&self) -> bool {
        match self.blocknum_blocks.last() {
            Some(last) => last.data_len < self.options.blksize() + HEADER_LEN,
            _ => false,
        }
    }

    pub fn reader(&self) -> &Mutex<BufReader<File>> {
        match self.local_file.as_ref() {
            Some(TftpSessionFile::Reader(reader)) => reader,
            _ => panic!(),
        }
    }

    pub fn set_reader(&mut self, file: File) {
        let reader = BufReader::new(file);
        self.local_file = Some(TftpSessionFile::Reader(Mutex::new(reader)));
    }

    pub fn writer_mut(&mut self) -> &mut BufWriter<File> {
        match self.local_file.as_mut() {
            Some(TftpSessionFile::Writer(writer)) => writer,
            _ => panic!(),
        }
    }

    pub fn set_writer(&mut self, file: File) {
        let writer = BufWriter::new(file);
        self.local_file = Some(TftpSessionFile::Writer(writer));
    }

    pub fn mode(&self) -> &str {
        &self.mode
    }

    pub fn set_mode(&mut self, mode: &str) {
        self.mode = mode.to_string();
    }

    pub fn options(&self) -> &Options {
        &self.options
    }

    pub fn set_options(&mut self, options: Options) {
        self.options = options;
    }

    pub fn rollover(&self) -> u32 {
        self.rollover
    }

    pub fn rollover_add(&mut self, value: u32) {
        self.rollover += value;
    }

    pub fn lastch(&self) -> Option<u8> {
        self.lastch
    }

    pub fn set_lastch(&mut self, lastch: Option<u8>) {
        self.lastch = lastch;
    }

    pub async fn write(&mut self, buf: &[u8]) -> Result<(usize, Option<u8>), Error> {
        let mode = self.mode().to_string();
        let lastch = self.lastch();
        file::write(self.writer_mut(), buf, &mode, lastch).await
    }

    async fn recv(&self, size: usize) -> Result<Bytes, Error> {
        self.retry_on_failed(|c| async {
            let mut buf = vec![0u8; size];
            let size = c.sock.recv(buf.as_mut_slice()).await?;
            buf.resize(size, 0);
            Ok(Bytes::from(buf))
        })
        .await
    }

    async fn recv_from(&self, size: usize) -> Result<(Bytes, SocketAddr), Error> {
        self.retry_on_failed(|c| async {
            let mut buf = vec![0u8; size];
            let (size, addr) = c.sock.recv_from(buf.as_mut_slice()).await?;
            buf.resize(size, 0);
            Ok((Bytes::from(buf), addr))
        })
        .await
    }

    pub async fn recv_with_timeout(&self, size: usize) -> Result<Bytes, Error> {
        let (_, ret) = self
            .wait_for_recv(|_| async { Ok(()) }, |c| c.recv(size))
            .await?;
        Ok(ret)
    }

    async fn send(&self, buf: &Bytes) -> Result<usize, Error> {
        self.retry_on_failed(|c| c.sock.send(buf)).await
    }

    async fn send_to(&self, buf: &Bytes, addr: &SocketAddr) -> Result<usize, Error> {
        self.retry_on_failed(|c| c.sock.send_to(buf, addr)).await
    }

    pub async fn send_ack(&self) -> Result<usize, Error> {
        trace!("[{}] send: ack #{}", self.remote_addr(), self.blocknum_ack);
        self.send(&packet::ack(self.blocknum_ack)).await
    }

    pub async fn send_error(&self, err: Error) -> Result<usize, Error> {
        trace!("[{}] send: error {:?}", self.remote_addr(), err);
        self.send(&packet::error(err)).await
    }

    pub async fn send_ack_recv_data(&self) -> Result<(usize, Bytes), Error> {
        self.wait_for_recv(
            |c| c.send_ack(),
            |c| c.recv(c.options().blksize() + HEADER_LEN),
        )
        .await
    }

    pub async fn send_data_recv_ack(
        &mut self,
        blocknum_start: u16,
    ) -> Result<(usize, Bytes), Error> {
        let blocknum_req = match blocknum_start.checked_add(1) {
            Some(v) => v,
            _ => ROLLOVER,
        };

        let block = self
            .blocknum_blocks
            .iter()
            .find(|b| b.blocknum == blocknum_req);
        let (reader_pos, lastch) = match block {
            Some(block) => (block.reader_pos, block.lastch),
            _ => match self.blocknum_blocks.last() {
                Some(last) => (last.reader_pos + (last.reader_pos_len as u64), self.lastch),
                _ => (0, None),
            },
        };

        let ((blocks, rollover, lastch), buf) = self
            .wait_for_recv(
                |c| c.send_multi_data(blocknum_start, reader_pos, lastch),
                |c| c.recv(c.options().blksize() + HEADER_LEN),
            )
            .await?;
        let sent_len = blocks.iter().fold(0, |s, b| s + b.data_len);

        self.blocknum_blocks = blocks;
        self.rollover = rollover;
        self.lastch = lastch;

        Ok((sent_len, buf))
    }

    pub async fn send_oack_recv_data(&self) -> Result<(usize, Bytes), Error> {
        let oack = packet::oack(self.options());
        trace!("[{}] send: oack {:?}", self.remote_addr(), self.options());
        self.wait_for_recv(
            |c| c.send(&oack),
            |c| c.recv(c.options().blksize() + HEADER_LEN),
        )
        .await
    }

    pub async fn send_req_recv_data(
        &mut self,
        req: &packet::Request,
    ) -> Result<(usize, Bytes), Error> {
        let req = packet::request(req);
        trace!("[{}] send: req {:?}", self.remote_addr(), req);
        let (size, (buf, addr)) = self
            .wait_for_recv(
                |c| c.send_to(&req, c.remote_addr()),
                |c| c.recv_from(c.options().blksize() + HEADER_LEN),
            )
            .await?;
        self.remote_addr = addr;

        self.sock.connect(self.remote_addr()).await?;

        Ok((size, buf))
    }

    async fn send_multi_data(
        &self,
        blocknum_start: u16,
        reader_pos: u64,
        lastch: Option<u8>,
    ) -> Result<(Vec<FileBlock>, u32, Option<u8>), Error> {
        let mut rollover = self.rollover;

        let mut blocknum_req = blocknum_start;
        let mut reader_pos = reader_pos;
        let mut lastch = lastch;

        let mut blocks = vec![];
        for _ in 0..self.options().windowsize() {
            blocknum_req = match blocknum_req.checked_add(1) {
                Some(v) => v,
                _ => {
                    rollover += 1;
                    ROLLOVER
                }
            };

            let mut data_buf = vec![0u8; self.options().blksize()];
            let reader_lock = self.reader();
            let mut reader = reader_lock.lock().await;
            let (reader_pos_len, data_buf_len, ch) = file::read(
                &mut reader,
                data_buf.as_mut_slice(),
                reader_pos,
                self.mode(),
                lastch,
            )
            .await?;

            trace!(
                "[{}] readed: block num #{} ({} bytes)",
                self.remote_addr(),
                blocknum_req,
                data_buf_len
            );

            let sent_len = self
                .send(&packet::data(
                    blocknum_req,
                    &data_buf.as_slice()[0..data_buf_len],
                ))
                .await?;
            let block = FileBlock {
                blocknum: blocknum_req,
                reader_pos,
                lastch,
                data_len: sent_len,
                reader_pos_len,
            };
            blocks.push(block);
            reader_pos += reader_pos_len as u64;
            lastch = ch;

            trace!(
                "[{}] sent: block num #{} ({} bytes)",
                self.remote_addr(),
                blocknum_req,
                sent_len
            );

            if sent_len < (self.options().blksize() + HEADER_LEN) {
                break;
            }
        }

        Ok((blocks, rollover, lastch))
    }

    async fn retry_on_failed<'a, Fut, T>(
        &'a self,
        action: impl Fn(&'a Self) -> Fut,
    ) -> Result<T, Error>
    where
        Fut: Future<Output = Result<T, std::io::Error>>,
    {
        let mut count = 1;
        loop {
            match action(self).await {
                Ok(ret) => {
                    return Ok(ret);
                }
                Err(err) => {
                    if count > 10 {
                        return Err(Error::from(err));
                    }

                    warn!("[{}] failed to send. retry", self.remote_addr());

                    time::sleep(Duration::from_millis(10)).await;

                    count += 1;
                }
            }
        }
    }

    async fn wait_for_recv<'a, SFut, S, RFut, R>(
        &'a self,
        send_action: impl Fn(&'a Self) -> SFut,
        recv_action: impl Fn(&'a Self) -> RFut,
    ) -> Result<(S, R), Error>
    where
        SFut: Future<Output = Result<S, Error>>,
        RFut: Future<Output = Result<R, Error>>,
    {
        let mut t = send_action(self).await?;

        let mut retransmit = 1;
        loop {
            if let Ok(task) = time::timeout(
                Duration::from_secs(self.options().timeout()),
                recv_action(self),
            )
            .await
            {
                return Ok((t, task?));
            }

            if retransmit >= 10 {
                return Err(Error::Timedout);
            }

            warn!(
                "[{}] timedout: {}s {}times",
                self.remote_addr(),
                self.options().timeout(),
                retransmit
            );

            t = send_action(self).await?;
            retransmit += 1;
        }
    }
}
