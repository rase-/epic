extern crate epic;

use std::io::{TcpListener, TcpStream};
use std::io::{Acceptor, Listener};
use std::str::from_utf8;
use std::collections::HashMap;
use std::thread::Thread;

fn main() {
    let tcp_listener = TcpListener::bind("127.0.0.1:3000");
    let mut acceptor = tcp_listener.listen();

    // Spawn HTTP server
    Thread::spawn(move || {
        for mut opt_stream in acceptor.incoming() {
            match opt_stream {
                Err(e) => println!("Error: {}", e),
                Ok(mut stream) => Thread::spawn(move || {
                    let req = epic::HttpParser::read_request(&mut stream);
                    println!("Req: {}", req);

                    // Write something back
                    stream.write(b"HTTP/1.1 200 VERY OK\r\nContent-Type: text/plain\r\nContent-Length:12\r\n\r\nHello");
                    stream.write(b" world!");
                }).detach()
            }
        }
    }).detach();

   let mut stream = TcpStream::connect("127.0.0.1:3000").unwrap();
   stream.write(b"GET /index.html HTTP/1.1\r\nContent-Type: text/plain\r\nContent-Length:12\r\nTransfer-Encoding: gzip, chunked\r\n\r\nHello").unwrap();
   stream.write(b" world!").unwrap();
   println!("Client got: {}", epic::HttpParser::read_response(&mut stream));
}
