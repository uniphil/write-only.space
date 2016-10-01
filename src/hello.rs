extern crate iron;

use std::str::FromStr;
use std::env;
use iron::{Iron, Request, Response, IronResult};
use iron::status;

fn hello(_: &mut Request) -> IronResult<Response> {
    let resp = Response::with((status::Ok, "helloyo"));
    Ok(resp)
}

fn get_server_port() -> u16 {
    let port_str = env::var("PORT").unwrap_or(String::new());
    FromStr::from_str(&port_str).unwrap_or(8080)
}

fn main() {
    let port = get_server_port();
    println!("Starting on port {}", port);
    let server = Iron::new(hello).http(("0.0.0.0", port));
    match server {
      Err(m) => println!("Failed to start: {}", m),
      _ => {},
    }
}
