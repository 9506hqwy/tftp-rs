use clap::{Arg, Command};
use std::net::Ipv4Addr;
use std::path::Path;
use std::str::FromStr;
use tftp::client::Client;
use tftp::error::Error;
use tftp::options::OptionBuilder;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let matches = Command::new("TFTP Client")
        .version("0.2.0")
        .arg(
            Arg::new("host")
                .value_name("HOST")
                .value_parser(check_type::<Ipv4Addr>)
                .required(true)
                .help("connect server's IP address."),
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .default_value("69")
                .value_name("PORT")
                .value_parser(check_type::<u16>)
                .help("connect server's port."),
        )
        .arg(
            Arg::new("remote_file")
                .value_name("REMOTE FILE")
                .required(true)
                .help("server's file path."),
        )
        .arg(
            Arg::new("local_file")
                .value_name("LOCAL FILE")
                .required(true)
                .help("local's file path."),
        )
        .arg(
            Arg::new("operation")
                .value_name("OPERATION")
                .value_parser(["get", "put"])
                .required(true)
                .help("operation."),
        )
        .arg(
            Arg::new("mode")
                .short('m')
                .long("mode")
                .default_value("netascii")
                .value_name("MODE")
                .value_parser(["netascii", "octet"])
                .help("transfer mode."),
        )
        .arg(
            Arg::new("blksize")
                .short('b')
                .long("blksize")
                .value_name("BLKSIZE")
                .value_parser(check_type::<u16>)
                .help("blksize."),
        )
        .arg(
            Arg::new("timeout")
                .short('t')
                .long("timeout")
                .value_name("TIMEOUT")
                .value_parser(check_type::<u8>)
                .help("timeout."),
        )
        .arg(Arg::new("tsize").long("tsize").num_args(0).help("tsize."))
        .arg(
            Arg::new("windowsize")
                .short('w')
                .long("windowsize")
                .value_name("WINDOWSIZE")
                .value_parser(check_type::<u16>)
                .help("windowsize."),
        )
        .get_matches();

    let address = matches.get_one::<Ipv4Addr>("host").unwrap();
    let port = matches.get_one::<u16>("port").unwrap();
    let remote = matches.get_one::<String>("remote_file").unwrap();
    let local = matches.get_one::<String>("local_file").unwrap();
    let op = matches.get_one::<String>("operation").unwrap();
    let mode = matches.get_one::<String>("mode").unwrap();

    let mut builder = OptionBuilder::default();

    if let Some(blksize) = matches.get_one::<u16>("blksize") {
        builder = builder.blksize(*blksize);
    }

    if let Some(timeout) = matches.get_one::<u8>("timeout") {
        builder = builder.timeout(*timeout);
    }

    if matches.get_flag("tsize") {
        builder = builder.tsize();
    }

    if let Some(windowsize) = matches.get_one::<u16>("windowsize") {
        builder = builder.windowsize(*windowsize);
    }

    let client = Client::new(
        format!("{}:{}", address, port).parse()?,
        mode,
        builder.build(),
    );

    match op.as_str() {
        "get" => client.get(Path::new(local), remote).await,
        "put" => client.put(Path::new(local), remote).await,
        _ => unimplemented!(),
    }
}

fn check_type<T>(value: &str) -> Result<T, String>
where
    T: FromStr,
{
    Ok(value.parse::<T>().map_err(|_| value)?)
}
