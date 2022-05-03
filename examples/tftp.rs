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
                .validator(check_type::<Ipv4Addr>)
                .required(true)
                .help("connect server's IP address."),
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .default_value("69")
                .value_name("PORT")
                .validator(check_type::<u16>)
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
                .possible_values(["get", "put"])
                .required(true)
                .help("operation."),
        )
        .arg(
            Arg::new("mode")
                .short('m')
                .long("mode")
                .default_value("netascii")
                .value_name("MODE")
                .possible_values(["netascii", "octet"])
                .help("transfer mode."),
        )
        .arg(
            Arg::new("blksize")
                .short('b')
                .long("blksize")
                .value_name("BLKSIZE")
                .validator(check_type::<u16>)
                .help("blksize."),
        )
        .arg(
            Arg::new("timeout")
                .short('t')
                .long("timeout")
                .value_name("TIMEOUT")
                .validator(check_type::<u8>)
                .help("timeout."),
        )
        .arg(
            Arg::new("tsize")
                .long("tsize")
                .takes_value(false)
                .help("tsize."),
        )
        .arg(
            Arg::new("windowsize")
                .short('w')
                .long("windowsize")
                .value_name("WINDOWSIZE")
                .validator(check_type::<u16>)
                .help("windowsize."),
        )
        .get_matches();

    let address = matches.value_of("host").unwrap();
    let port = matches.value_of("port").unwrap();
    let remote = matches.value_of("remote_file").unwrap();
    let local = matches.value_of("local_file").unwrap();
    let op = matches.value_of("operation").unwrap();
    let mode = matches.value_of("mode").unwrap();

    let mut builder = OptionBuilder::default();

    if let Some(blksize) = matches.value_of("blksize") {
        builder = builder.blksize(blksize.parse().unwrap());
    }

    if let Some(timeout) = matches.value_of("timeout") {
        builder = builder.timeout(timeout.parse().unwrap());
    }

    if matches.is_present("tsize") {
        builder = builder.tsize();
    }

    if let Some(windowsize) = matches.value_of("windowsize") {
        builder = builder.windowsize(windowsize.parse().unwrap());
    }

    let client = Client::new(
        format!("{}:{}", address, port).parse()?,
        mode,
        builder.build(),
    );

    match op {
        "get" => client.get(Path::new(local), remote).await,
        "put" => client.put(Path::new(local), remote).await,
        _ => unimplemented!(),
    }
}

fn check_type<T>(value: &str) -> Result<(), String>
where
    T: FromStr,
{
    value.parse::<T>().map_err(|_| value)?;
    Ok(())
}
