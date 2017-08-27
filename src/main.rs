extern crate ipvs;

use ipvs::Client;

fn main() {
    let mut client = Client::new().unwrap();
    println!("{}", client.family());
    client.flush().unwrap();
}
