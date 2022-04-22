use super::error::Error;
use super::file;
use super::handle_packet;
use super::options::Options;
use super::packet;
use super::session;
use std::net::SocketAddr;
use std::path::Path;
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
        file::create(local_file).await?;

        let req = packet::Request::rrq(remote_file, &self.mode, &self.options);

        self.handl_request(req, local_file).await
    }

    pub async fn put(&self, local_file: &Path, remote_file: &str) -> Result<(), Error> {
        let local = local_file.canonicalize()?;

        let mut req = packet::Request::wrq(remote_file, &self.mode, &self.options);
        req.options_mut().set_tsize(&local);

        self.handl_request(req, &local).await
    }

    async fn handl_request(&self, req: packet::Request, filepath: &Path) -> Result<(), Error> {
        let sock = UdpSocket::bind("0.0.0.0:0").await?;

        let mut session = session::TftpSession::new(sock, self.remote_addr);
        session.set_mode(req.mode());

        let (_, buf) = session.send_req_recv_data(&req).await?;

        handle_packet(req.op_code(), &mut session, buf, filepath).await?;

        Ok(())
    }
}
