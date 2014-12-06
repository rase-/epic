use std::io::{TcpListener, TcpStream};
use std::io::{Acceptor, Listener};
use std::str::from_utf8;

#[test]
fn it_works() {
    let tcp_listener = TcpListener::bind("127.0.0.1:3000");
    let mut acceptor = tcp_listener.listen();

    // Spawn HTTP server
    spawn(proc() {
        for mut opt_stream in acceptor.incoming() {
            match opt_stream {
                Err(e) => println!("Error: {}", e),
                Ok(mut stream) => spawn(proc() {
                    loop {
                        let mut buf = [0u8, ..4096];
                        let count = stream.read(&mut buf).unwrap_or(0);

                        if 0 == count {
                            break;
                        }

                        let slice = buf.slice(0, count);
                        let msg = from_utf8(slice).unwrap_or("");

                        println!("server got: {}", msg);

                        stream.write(slice);
                    }
                })
            }
        }
    });

    let mut stream = TcpStream::connect("127.0.0.1:3000");
    stream.write(b"Hello World!\r\n").unwrap();
    println!("aoeaoeaoe");

    let mut buf = [0u8, ..4096];
    let count = stream.read(&mut buf);
    let msg = from_utf8(&buf).unwrap_or("");
    println!("client got: {}", msg);
}
