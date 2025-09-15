use clap::{Arg, Command};
use std::net::Ipv4Addr;
use std::path::Path;
use std::str::FromStr;
use tftp::error::Error;
use tftp::options::OptionBuilder;
use tftp::server::Server;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let matches = Command::new("TFTP Server")
        .version("0.2.0")
        .arg(
            Arg::new("bind")
                .short('i')
                .long("bind")
                .default_value("0.0.0.0")
                .value_name("IPADDRESS")
                .value_parser(check_type::<Ipv4Addr>)
                .help("bind server's IP address."),
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .default_value("69")
                .value_name("PORT")
                .value_parser(check_type::<u16>)
                .help("bind server's port."),
        )
        .arg(
            Arg::new("root")
                .short('r')
                .long("root")
                .default_value(".")
                .value_name("PATH")
                .value_parser(check_root)
                .help("publish TFTP root directory."),
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
                .num_args(0)
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

    let address = matches.get_one::<Ipv4Addr>("bind").unwrap();
    let port = matches.get_one::<u16>("port").unwrap();
    let root = matches.get_one::<String>("root").unwrap();

    let mut builder = OptionBuilder::default();

    if let Some(blksize) = matches.get_one::<u16>("blksize") {
        builder = builder.blksize(*blksize);
    }

    if matches.get_flag("timeout") {
        builder = builder.timeout(0);
    }

    if matches.get_flag("tsize") {
        builder = builder.tsize();
    }

    if let Some(windowsize) = matches.get_one::<u16>("windowsize") {
        builder = builder.windowsize(*windowsize);
    }

    let server = Server::new(
        format!("{address}:{port}").parse()?,
        Path::new(root),
        builder.build(),
    )?;
    server.serve_forever().await?;
    Ok(())
}

fn check_root(root: &str) -> Result<String, String> {
    let path = Path::new(&root);
    if path.is_dir() {
        Ok(root.to_string())
    } else {
        Err(root.to_string())
    }
}

fn check_type<T>(value: &str) -> Result<T, String>
where
    T: FromStr,
{
    Ok(value.parse::<T>().map_err(|_| value)?)
}
