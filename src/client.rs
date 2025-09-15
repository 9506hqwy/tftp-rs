use super::OpCode;
use super::error::Error;
use super::file;
use super::handle_packet;
use super::options::Options;
use super::packet;
use super::session;
use std::net::SocketAddr;
use std::path::Path;
use tokio::fs::File;
use tokio::net::UdpSocket;

pub struct Client {
    remote_addr: SocketAddr,
    mode: String,
    options: Options,
}

impl Client {
    pub fn new(remote_addr: SocketAddr, mode: &str, options: Options) -> Client {
        Client {
            remote_addr,
            mode: mode.to_string(),
            options,
        }
    }

    pub async fn get(&self, local_file: &Path, remote_file: &str) -> Result<(), Error> {
        let local = file::open_create(local_file).await?;

        let req = packet::Request::rrq(remote_file, &self.mode, &self.options);

        self.handl_request(req, local).await
    }

    pub async fn put(&self, local_file: &Path, remote_file: &str) -> Result<(), Error> {
        let local_file = local_file.canonicalize()?;
        let local = file::open_read(&local_file).await?;

        let mut req = packet::Request::wrq(remote_file, &self.mode, &self.options);
        req.options_mut().set_tsize(&local_file);

        self.handl_request(req, local).await
    }

    async fn handl_request(&self, req: packet::Request, file: File) -> Result<(), Error> {
        let sock = UdpSocket::bind("0.0.0.0:0").await?;

        let mut session = session::TftpSession::new(sock, self.remote_addr);
        session.set_mode(req.mode());
        match *req.op_code() {
            OpCode::Rrq => session.set_writer(file),
            OpCode::Wrq => session.set_reader(file),
            _ => panic!(),
        }

        let (_, buf) = session.send_req_recv_data(&req).await?;

        handle_packet(req.op_code(), &mut session, buf).await?;

        Ok(())
    }
}
