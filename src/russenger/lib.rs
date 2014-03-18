#[crate_id = "russenger#0.1"];
#[comment = "A lightweight messaging layer on top of TCP"];
#[license = "MIT/ASL2"];
#[crate_type = "lib"];

extern crate msgpack;
extern crate collections;
extern crate serialize;
extern crate sync;

use std::io::BufferedStream;
use std::io::{Acceptor, Listener};
use std::io::net::ip::SocketAddr;
use std::io::net::tcp::{TcpListener, TcpStream};

use collections::hashmap::HashMap;
use msgpack::{to_msgpack, from_msgpack, Encoder, Decoder};
use serialize::{Encodable, Decodable};
use sync::MutexArc;

/// Create a `(tx, rx)` pair, where the port is used to receive all messages sent
/// to the given address, and the chan is used to send messages from this address.
/// Note that the messages are tuples of the form `(SocketAddr, T)`, where the `SocketAddr`
/// specifies where the messages comes from or goes to, and `T` is an arbitrary data type
/// that is encodable and decodable.
/// 
/// The spawned tasks will terminate after the said port and chan are destructed.
pub fn new<'a, T: Send + Encodable<Encoder<'a>> + Decodable<Decoder<'a>>>(addr: SocketAddr)
    -> (Sender<(SocketAddr, T)>, Receiver<(SocketAddr, T)>) {
    // For incoming messages
    let (in_tx, in_rx) = channel();
    // For outgoing messages
    let (out_tx, out_rx) = channel();
    spawn(proc() {
        run(addr, in_tx, out_rx);
    });
    return (out_tx, in_rx);
}

fn run<'a, T: Send + Encodable<Encoder<'a>> + Decodable<Decoder<'a>>>
    (addr: SocketAddr, in_tx: Sender<(SocketAddr, T)>, out_rx: Receiver<(SocketAddr, T)>) {
    // A stream map is a HashMap from SocketAddr to TcpStream.  The map is protected
    // by a RWArc, so to enable concurrent access.  Having this map allows us to multiplex
    // messages over TcpStreams.
    let stream_map_arc = MutexArc::new(HashMap::new());

    // The following task is responsible for:
    // 1. Accept new connections
    // 2. Receive messages from those connections
    // 3. Forward the messages to the receiver
    let stream_map_arc_clone = stream_map_arc.clone();
    let in_tx_clone = in_tx.clone();
    spawn(proc() {
        let stream_map_arc = stream_map_arc_clone;
        let mut acceptor = TcpListener::bind(addr).listen();
        // TODO: how do you terminate this task?
        for stream in acceptor.incoming() {
            let mut stream = stream.unwrap();
            let remote_addr = stream.peer_name().unwrap();
            stream_map_arc.access(|stream_map| {
                stream_map.insert(remote_addr, stream.clone());
            });
            spawn(proc() keep_reading(stream, in_tx_clone, remote_addr));
        }
    });

    // This infinite loop receives messages from the user and send them
    // over the network.
    loop {
        let (to, msg) = match out_rx.recv_opt() {
            Some(data) => data,
            None => return // the other end has hung up
        };
        let mut stream = match { stream_map_arc.access(|stream_map| {
            match stream_map.find_copy(&to) {
                Some(stream) => Some(stream),
                None => {
                    println!("didn't find a stream");
                    let stream = match TcpStream::connect(to) {
                        Ok(stream) => stream,
                        Err(..) => return None
                    };
                    stream_map.insert(to, stream.clone());
                    let reader = stream.clone();
                    let in_tx_clone = in_tx.clone();
                    spawn(proc() keep_reading(reader, in_tx_clone, to));
                    Some(stream)
                }
            }
        }) } {
            Some(stream) => stream,
            None => continue
        };
        let msg_bytes = to_msgpack(&msg);
        let msg_size = msg_bytes.len();
        // TODO: think about how to deal with these failures...
        // we shouldn't just call fail!()
        match stream.write_le_u32(msg_size as u32) {
            Ok(..) => (),
            Err(err) => fail!("{}", err.to_str()),
        };
        match stream.write(msg_bytes) {
            Ok(..) => (),
            Err(err) => fail!("{}", err.to_str()),
        };
    }

    fn keep_reading<'a, T: Send + Encodable<Encoder<'a>> + Decodable<Decoder<'a>>>
        (stream: TcpStream, in_tx: Sender<(SocketAddr, T)>, addr: SocketAddr) {
        let mut stream = BufferedStream::new(stream);
        loop {
            let msg_size = stream.read_le_u32().unwrap();
            let msg_bytes = stream.read_bytes(msg_size as uint).unwrap();
            if !in_tx.try_send((addr, from_msgpack(msg_bytes))) {
                // Terminate if the other end has hung up
                return;
            }
        }
    }
}