pub mod client;
pub mod error;
pub mod options;
pub mod server;

mod file;
mod packet;
mod session;

use self::error::Error;
use bytes::Bytes;
use log::{error, trace};
use std::cmp::Ordering;

const HEADER_LEN: usize = 4;
const ROLLOVER: u16 = 0;

#[derive(Clone, Debug)]
pub enum OpCode {
    Rrq = 1,
    Wrq = 2,
    Data = 3,
    Ack = 4,
    Error = 5,
    Oack = 6,
}

#[derive(Clone)]
pub enum ErrorCode {
    NotDefined = 0,
    FileNotFound = 1,
    AccessViolation = 2,
    DiskFull = 3,
    IllegalTftpOp = 4,
    UnknownTId = 5,
    FileAlreadyExists = 6,
    NoSuchUser = 7,
    OptionNotSupport = 8,
}

async fn handle_ack(
    session: &mut session::TftpSession,
    ack: &mut Bytes,
) -> Result<Option<Bytes>, Error> {
    let blocknum = packet::parse_blocknum(ack)?;

    trace!(
        "[{}] received: ACK block num #{} (#{})",
        session.remote_addr(),
        blocknum,
        session.blocknum_ack()
    );

    if blocknum != 0 || session.rollover() != 0 {
        if !session.blocknum_expect(blocknum) {
            // 期待したブロックでなければ再度待ち受ける。
            let rev_buf = session
                .recv_with_timeout(session.options().blksize() + HEADER_LEN)
                .await?;
            return Ok(Some(rev_buf));
        }

        session.set_blocknum_ack(blocknum);

        if session.sent_completed() {
            return Ok(None);
        }
    }

    let (_, buf) = session.send_data_recv_ack(session.blocknum_ack()).await?;
    Ok(Some(buf))
}

async fn handle_data(
    session: &mut session::TftpSession,
    data: &mut Bytes,
) -> Result<Option<Bytes>, Error> {
    let blocknum = packet::parse_blocknum(data)?;

    trace!(
        "[{}] received: DATA block num #{} (#{})",
        session.remote_addr(),
        blocknum,
        session.blocknum_ack()
    );

    let blocknum_expect = session.blocknum_ack_add(1);
    match blocknum_expect.cmp(&blocknum) {
        Ordering::Less => {
            // 期待したブロックよりも先のブロックを受け取った。
            let (_, buf) = session.send_ack_recv_data().await?;
            session.received_data_clear();
            Ok(Some(buf))
        }
        Ordering::Equal => {
            session.received_data_inc();

            if blocknum_expect == ROLLOVER {
                session.rollover_add(1);
            }

            let (_, lastch) = session.write(data.as_ref()).await?;
            session.set_lastch(lastch);

            // データの保存が成功したら ACK を更新する。
            session.set_blocknum_ack(blocknum);

            if data.len() < session.options().blksize() {
                session.send_ack().await?;
                return Ok(None);
            }

            if session.received_data_last() {
                // Window Size 分を受け取れば ACK を送信する。
                let (_, buf) = session.send_ack_recv_data().await?;
                session.received_data_clear();
                Ok(Some(buf))
            } else {
                let buf = session
                    .recv_with_timeout(session.options().blksize() + HEADER_LEN)
                    .await?;
                Ok(Some(buf))
            }
        }
        Ordering::Greater => {
            // 期待したブロックよりも前のブロックの場合は無視する。
            let buf = session
                .recv_with_timeout(session.options().blksize() + HEADER_LEN)
                .await?;
            Ok(Some(buf))
        }
    }
}

fn handle_error(
    session: &mut session::TftpSession,
    error: &mut Bytes,
) -> Result<Option<Bytes>, Error> {
    let error = packet::parse_error(error)?;
    error!(
        "[{}] {}: {}",
        session.remote_addr(),
        error.error_code(),
        error.message()
    );
    Ok(None)
}

async fn handle_oack(
    session: &mut session::TftpSession,
    req_code: &OpCode,
    oack: &mut Bytes,
) -> Result<Option<Bytes>, Error> {
    // クライアントのみ。
    let options = packet::parse_oack(oack)?;
    session.set_options(options);

    let (_, buf) = match req_code {
        &OpCode::Wrq => session.send_data_recv_ack(0).await,
        _ => {
            if session.options().tsize() != 0 {
                // TODO: check ErrorCode::DiskFull
            }

            session.send_ack_recv_data().await
        }
    }?;

    Ok(Some(buf))
}

async fn handle_packet(
    req_code: &OpCode,
    session: &mut session::TftpSession,
    mut buf: Bytes,
) -> Result<(), Error> {
    loop {
        let op_code = packet::parse_opcode(&mut buf)?.ok_or(Error::InvalidOpCode)?;

        let ret = match op_code {
            OpCode::Ack => handle_ack(session, &mut buf).await,
            OpCode::Data => handle_data(session, &mut buf).await,
            OpCode::Oack => handle_oack(session, req_code, &mut buf).await,
            OpCode::Error => handle_error(session, &mut buf),
            _ => return Err(Error::InvalidOpCode),
        }?;

        match ret {
            Some(tmp) => buf = tmp,
            _ => break,
        }
    }

    trace!("[{}] completed: {:?}", session.remote_addr(), req_code,);

    Ok(())
}
