extern crate ipvs;
extern crate env_logger;

use ipvs::Client;

fn main() {
    env_logger::init();

    let mut client = Client::new().unwrap();
    println!("{}", client.family());
    client.flush().unwrap();
}
