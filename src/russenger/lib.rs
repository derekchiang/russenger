#[crate_id = "russenger#0.1"];
#[comment = "A lightweight messaging layer on top of TCP"];
#[license = "MIT/ASL2"];
#[crate_type = "lib"];

extern mod msgpack;
extern mod serialize;
extern mod sync;

use std::hashmap::HashMap;
use std::io::BufferedStream;
use std::io::{Acceptor, Listener};
use std::io::net::ip::SocketAddr;
use std::io::net::tcp::{TcpListener, TcpStream};

use msgpack::{to_msgpack, from_msgpack, Encoder, Decoder};
use serialize::{Encodable, Decodable};
use sync::MutexArc;

/// Create a `(port, chan)` pair, where the port is used to receive all messages sent
/// to the given address, and the chan is used to send messages from this address.
/// Note that the messages are tuples of the form `(SocketAddr, T)`, where the `SocketAddr`
/// specifies where the messages comes from or goes to, and `T` is an arbitrary data type
/// that is encodable and decodable.
/// 
/// The spawned tasks will terminate after the said port and chan are destructed.
pub fn new<'a, T: Send + Freeze + Encodable<Encoder<'a>> + Decodable<Decoder<'a>>>(addr: SocketAddr)
    -> (Port<(SocketAddr, T)>, Chan<(SocketAddr, T)>) {
    // For incoming messages
    let (in_port, in_chan) = Chan::new();
    // For outgoing messages
    let (out_port, out_chan) = Chan::new();
    spawn(proc() {
        run(addr, in_chan, out_port);
    });
    return (in_port, out_chan);
}

fn run<'a, T: Send + Freeze + Encodable<Encoder<'a>> + Decodable<Decoder<'a>>>
    (addr: SocketAddr, in_chan: Chan<(SocketAddr, T)>, out_port: Port<(SocketAddr, T)>) {
    // A stream map is a HashMap from SocketAddr to TcpStream.  The map is protected
    // by a RWArc, so to enable concurrent access.  Having this map allows us to multiplex
    // messages over TcpStreams.
    let stream_map_arc = MutexArc::new(HashMap::new());

    // The following task is responsible for:
    // 1. Accept new connections
    // 2. Receive messages from those connections
    // 3. Forward the messages to the receiver
    let stream_map_arc_clone = stream_map_arc.clone();
    spawn(proc() {
        let stream_map_arc = stream_map_arc_clone;
        let mut acceptor = TcpListener::bind(addr).listen();
        // TODO: how do you terminate this task?
        for stream in acceptor.incoming() {
            let mut stream = stream.unwrap();
            let remote_addr = stream.peer_name().unwrap();
            unsafe {
                stream_map_arc.unsafe_access(|stream_map| {
                    stream_map.insert(remote_addr, stream.clone());
                });
            }
            let in_chan_clone = in_chan.clone();
            spawn(proc() {
                let mut stream = BufferedStream::new(stream);
                let msg_size = stream.read_le_u32().unwrap();
                let msg_bytes = stream.read_bytes(msg_size as uint).unwrap();
                if !in_chan_clone.try_send((remote_addr, from_msgpack(msg_bytes))) {
                    // Terminate if the other end has hung up
                    return;
                }
            });
        }
    });

    // This infinite loop receives messages from the user and send them
    // over the network.
    loop {
        let (to, msg) = match out_port.recv_opt() {
            Some(data) => data,
            None => return // the other end has hung up
        };
        let mut stream = unsafe {
            stream_map_arc.unsafe_access(|stream_map| {
                match stream_map.find_copy(&to) {
                    Some(stream) => stream,
                    None => {
                        let stream = TcpStream::connect(to).unwrap();
                        stream_map.insert(to, stream.clone());
                        stream
                    }
                }
            })
        };
        let msg_bytes = to_msgpack(&msg);
        let msg_size = msg_bytes.len();
        stream.write_le_u32(msg_size as u32);
        stream.write(msg_bytes);
    }
}