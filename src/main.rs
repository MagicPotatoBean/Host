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
/// Simple program to greet a person
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
fn main() {
    let args = Args::parse();
    let this_dir = current_dir().expect("Failed to get current directory");
    let mut files = HashMap::new();
    print!("Hosting:\n");
    for file in args.files.chunks(2) {
        let dir = this_dir.join(&file[0]);
        if !file[1].as_path().starts_with("/") {
            panic!("File display names must start with a \"/\" i.e. /file.txt, not just file.txt")
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
}
fn host_files(address: SocketAddr, files: Arc<HashMap<PathBuf, PathBuf>>) -> std::io::Result<()> {
    let mut listener = TcpListener::bind(address).map_err(|err| {
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
        std::thread::Builder::new()
            .name(format!("{address}"))
            .spawn(move || send_file(request, cloned_files))
            .unwrap();
    }
    unreachable!()
}
fn send_file(mut stream: TcpStream, files: Arc<HashMap<PathBuf, PathBuf>>) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_millis(100)))?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf);
    let request = String::from_utf8_lossy(&buf);
    println!("{request}");
    let request_line = request.lines().next().unwrap();
    let path = request_line
        .split_once(" ")
        .unwrap()
        .1
        .split_once(" ")
        .unwrap()
        .0;
    println!(
        "Requested: {}",
        files.get(&PathBuf::from(path)).unwrap().display()
    );
    stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n")?;
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .open(files.get(&PathBuf::from(path)).unwrap())
        .unwrap();
    loop {
        let mut buf = [0; 1024];
        match file.read(&mut buf) {
            Ok(amount_read) => {
                if amount_read == 0 {
                    break;
                }
                stream.write_all(&buf[0..amount_read]);
            }
            Err(_) => {
                println!("ERRRR");
                break;
            }
        }
    }
    stream.shutdown(std::net::Shutdown::Both)
}
