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
                .validator(check_type::<Ipv4Addr>)
                .help("bind server's IP address."),
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .default_value("69")
                .value_name("PORT")
                .validator(check_type::<u16>)
                .help("bind server's port."),
        )
        .arg(
            Arg::new("root")
                .short('r')
                .long("root")
                .default_value(".")
                .value_name("PATH")
                .validator(check_root)
                .help("publish TFTP root directory."),
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
                .takes_value(false)
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

    let address = matches.value_of("bind").unwrap();
    let port = matches.value_of("port").unwrap().parse::<u16>().unwrap();
    let root = matches.value_of("root").unwrap();

    let mut builder = OptionBuilder::default();

    if let Some(blksize) = matches.value_of("blksize") {
        builder = builder.blksize(blksize.parse().unwrap());
    }

    if matches.is_present("timeout") {
        builder = builder.timeout(0);
    }

    if matches.is_present("tsize") {
        builder = builder.tsize();
    }

    if let Some(windowsize) = matches.value_of("windowsize") {
        builder = builder.windowsize(windowsize.parse().unwrap());
    }

    let server = Server::new(
        format!("{0}:{1}", address, port).parse()?,
        Path::new(root),
        builder.build(),
    )?;
    server.serve_forever().await?;
    Ok(())
}

fn check_root(root: &str) -> Result<(), String> {
    let path = Path::new(&root);
    if path.is_dir() {
        Ok(())
    } else {
        Err(root.to_string())
    }
}

fn check_type<T>(value: &str) -> Result<(), String>
where
    T: FromStr,
{
    value.parse::<T>().map_err(|_| value)?;
    Ok(())
}
