extern crate russenger;
extern crate serialize;

use std::io::timer::sleep;

#[deriving(Encodable, Decodable, Show)]
struct Message {
    id: uint,
    content: ~str
}

fn main() {
    let addrA = from_str("127.0.0.1:4005").unwrap();
    let addrB = from_str("127.0.0.1:4010").unwrap();

    spawn(proc() {
        let (port, chan) = russenger::new(addrB);
        sleep(100);
        let (from, msg) = port.recv();
        println!("{} from {}", msg, from.to_str());
        chan.send((from, Message {
            id: 20,
            content: ~"Yo"
        }));
    });

    let (port, chan) = russenger::new(addrA);
    sleep(100);
    let msg = Message {
        id: 10,
        content: ~"Hello"
    };
    chan.send((addrB, msg));
    let (from, reply) = port.recv();
    println!("{} from {}", reply, from.to_str());
}