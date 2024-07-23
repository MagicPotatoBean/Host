#!/usr/bin/env nix-shell
//! ```cargo
//! [dependencies]
//! clap = {version = "4.5.8", features = ["derive"]}
//! anyhow = "1.0.86"
//! color-eyre = "0.6.3"
//! ```

use anyhow::{anyhow, bail, Context};
use clap::Parser;
use std::{
    collections::HashMap,
    env::current_dir,
    io::{BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
/*
#!nix-shell -i rust-script -p rustc -p rust-script -p cargo
*/
type Result<T> = anyhow::Result<T>;
/// Simple program to host files
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The files to host (in the form "/path/to/file /download/path")
    /// i.e. "-f /path/to/myfile.txt /myfile.txt"
    #[arg(short, long,verbatim_doc_comment, value_parser, num_args = 1.., value_delimiter = ' ', required = true)]
    files: Vec<PathBuf>,

    /// The addresses to host the files on (ip:port)
    #[arg(short, long, value_parser, num_args = 1.., value_delimiter = ' ', required = true)]
    addresses: Vec<SocketAddr>,
}
pub fn main() -> Result<()> {
    let args = Args::parse();
    let this_dir = current_dir().expect("Failed to get current directory");
    let mut files = HashMap::new();
    print!("Hosting:\n");
    for file in args.files.chunks(2) {
        if file.len() != 2 {
            bail!("Files must be paired with a \"display name\", which will be used as the user will access the files with.\n i.e. ... -f /home/user/Downloads/thisfile.txt /thisfile.txt ...")
        }
        let dir = this_dir.join(&file[0]);
        if !file[1].as_path().starts_with("/") {
            bail!("File display names must start with a \"/\" i.e. /file.txt, not just file.txt");
        }
        print!("    {}  as  {}\n", dir.display(), file[1].display());
        files.insert(file[1].to_path_buf(), dir);
    }
    print!("\nOn:\n");
    let file_list = Arc::new(files);
    for address in args.addresses {
        print!("    {address}\n");
        let cloned_files = Arc::clone(&file_list);
        std::thread::Builder::new()
            .name(format!("{address}"))
            .spawn(move || host_files(address, cloned_files))
            .expect("Failed to spawn required threads.");
    }
    println!("\nPress [ENTER] to close server.");
    let _ = std::io::stdin().read_line(&mut String::default());
    Ok(())
}
fn host_files(address: SocketAddr, files: Arc<HashMap<PathBuf, PathBuf>>) -> std::io::Result<()> {
    let listener = TcpListener::bind(address).map_err(|err| {
        println!("{}", 
            match err.kind() {
                std::io::ErrorKind::PermissionDenied => format!("Failed to bind to address {address} due to insufficient permission. Try running as sudo/administrator."),
                std::io::ErrorKind::AddrInUse => format!("Failed to bind to address {address} since it is already in use. Try stopping the process already using this address."),
                std::io::ErrorKind::AddrNotAvailable => format!("Failed to bind to address {address} since it doesn't exist. Try setting the address to a real interface that is owned by this device."),
                _ => format!("Failed to bind to address {address}: {err}"),
            }
        );
        err
    })?;
    for request in listener.incoming().flatten() {
        let cloned_files = Arc::clone(&files);
        if std::thread::Builder::new()
            .name(format!("{address}"))
            .spawn(move || send_file(request, cloned_files))
            .is_err()
        {
            println!("Encountered an error when spawning a thread to host a file, continuing.")
        };
    }
    unreachable!()
}
fn send_file(mut stream: TcpStream, files: Arc<HashMap<PathBuf, PathBuf>>) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_millis(100)))?;
    let mut buf = Vec::new();
    let _ = stream.read_to_end(&mut buf);
    let request = String::from_utf8_lossy(&buf);
    println!("{request}");
    let request_line = request
        .lines()
        .next()
        .ok_or(anyhow!("HTTP Request line was malformed"))?;
    let path = request_line
        .split_once(" ")
        .ok_or(anyhow!("HTTP Request line was malformed"))?
        .1
        .split_once(" ")
        .ok_or(anyhow!("HTTP Request line was malformed"))?
        .0;
    println!(
        "Requested: {}",
        files
            .get(&PathBuf::from(path))
            .ok_or(anyhow!("HTTP Request line was malformed"))?
            .display()
    );
    stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n")?;
    let path = files
        .get(&PathBuf::from(path))
        .ok_or(anyhow!("HTTP Request line was malformed"))?;
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .open(&path)
        .map_err(|err| {
            println!("No such file \"{}\".", path.display());
            err
        })?;
    loop {
        let mut buf = [0; 1024];
        match file.read(&mut buf) {
            Ok(amount_read) => {
                if amount_read == 0 {
                    break;
                }
                let _ = stream.write_all(&buf[0..amount_read]);
            }
            Err(_) => {
                println!("ERRRR");
                break;
            }
        }
    }
    println!("Successfully sent {}", path.display());
    Ok(stream.shutdown(std::net::Shutdown::Both)?)
}
