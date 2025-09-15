use super::error::Error;
use super::file;
use super::options::Options;
use super::packet;
use super::session;
use super::{OpCode, handle_packet};
use bytes::Bytes;
use log::{error, trace};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use tokio::net::UdpSocket;

#[derive(Debug)]
pub struct Server {
    service_addr: SocketAddr,
    root: PathBuf,
    options: Options,
}

impl Server {
    pub fn new(service_addr: SocketAddr, root: &Path, options: Options) -> Result<Server, Error> {
        Ok(Server {
            service_addr,
            root: root.canonicalize()?,
            options,
        })
    }

    pub async fn serve_forever(self) -> Result<(), Error> {
        let service_sock = UdpSocket::bind(self.service_addr).await?;

        trace!("serving: {:?}", &self);

        loop {
            let mut buf = vec![0; 1024];
            let (size, remote_addr) = service_sock.recv_from(buf.as_mut_slice()).await?;
            buf.resize(size, 0);

            let root = self.root.clone();
            let options = self.options.clone();
            tokio::spawn(async move {
                match UdpSocket::bind((self.service_addr.ip(), 0)).await {
                    Ok(sock) => {
                        if let Err(e) = sock.connect(remote_addr).await {
                            eprint!("[{remote_addr}] {e:?}");
                            return;
                        }

                        let mut session = session::TftpSession::new(sock, remote_addr);
                        if let Err(e) =
                            handle_request(&mut session, Bytes::from(buf), root.as_path(), options)
                                .await
                        {
                            if let Err(e) = session.send_error(e).await {
                                error!("failed to send error: [{}] {:?}", remote_addr, e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("failed to bind: [{}] {:?}", remote_addr, e);
                    }
                }
            });
        }
    }
}

async fn handle_request(
    session: &mut session::TftpSession,
    mut buf: Bytes,
    root: &Path,
    limitations: Options,
) -> Result<(), Error> {
    let req = packet::parse_request(&mut buf)?;
    session.set_mode(req.mode());

    trace!("requested: {:?}", &req);

    let mut filepath = PathBuf::from(root);
    filepath.push(req.filename());

    match req.op_code() {
        OpCode::Rrq => {
            let local_file = filepath.canonicalize()?;
            if !local_file.starts_with(root) {
                return Err(Error::InvalidFileName);
            }

            let local = file::open_read(&local_file).await?;
            session.set_reader(local);

            let mut options = req.options().clone();
            options.cut_off(&limitations);
            options.set_tsize(&local_file);
            session.set_options(options);

            let (_, buf) = if session.options().has_option() {
                session.send_oack_recv_data().await?
            } else {
                session.send_data_recv_ack(0).await?
            };

            handle_packet(req.op_code(), session, buf).await?;
        }
        OpCode::Wrq => {
            if (!filepath.starts_with(root)) || filepath.iter().any(|i| i == "..") {
                return Err(Error::InvalidFileName);
            }

            let local = file::open_create(&filepath).await?;
            session.set_writer(local);

            let mut options = req.options().clone();
            options.cut_off(&limitations);
            session.set_options(options);

            // TODO: check ErrorCode::DiskFull

            let (_, buf) = if session.options().has_option() {
                session.send_oack_recv_data().await?
            } else {
                session.send_ack_recv_data().await?
            };

            handle_packet(req.op_code(), session, buf).await?;
        }
        _ => {
            return Err(Error::InvalidOpCode);
        }
    }

    Ok(())
}
