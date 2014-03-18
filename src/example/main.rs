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
        let (tx, rx) = russenger::new(addrB);
        sleep(100);
        let (from, msg) = rx.recv();
        println!("{} from {}", msg, from.to_str());
        tx.send((from, Message {
            id: 20,
            content: ~"Yo"
        }));
    });

    let (tx, rx) = russenger::new(addrA);
    sleep(100);
    let msg = Message {
        id: 10,
        content: ~"Hello"
    };
    tx.send((addrB, msg));
    let (from, reply) = rx.recv();
    println!("{} from {}", reply, from.to_str());
}