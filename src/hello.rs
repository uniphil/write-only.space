extern crate chrono;
extern crate crypto;
extern crate iron;
extern crate logger;
extern crate params;
extern crate persistent;
extern crate postgres;
extern crate r2d2;
extern crate r2d2_postgres;
extern crate router;

use iron::{Iron, Chain, Request, Response, IronResult, Plugin, status};
use iron::mime::Mime;
use iron::typemap::Key;
use logger::Logger;
use params::{FromValue};
use persistent::Read as PRead;
use r2d2_postgres::{SslMode, PostgresConnectionManager};
use router::Router;

mod db;
mod migrate;

type PostgresPool = r2d2::Pool<PostgresConnectionManager>;

struct PostgresDB;
impl Key for PostgresDB { type Value = PostgresPool; }


fn base(content: &str) -> String {
    format!("<!doctype html>
        <html>
            <head>
                <title>write-only.space</title>
            </head>
            <body>
                {}
            </body>
        </html>
        ", content)
}

fn index(req: &mut Request) -> IronResult<Response> {
    let pool = req.get::<persistent::Read<PostgresDB>>().unwrap();
    let conn = pool.get().unwrap();
    let posts = conn
        .query("SELECT id, timestamp, sender, thread, body FROM post", &[])
        .unwrap()
        .iter()
        .map(|row| {
            let timestamp: chrono::NaiveDateTime = row.get("timestamp");
            let sender: String = row.get("sender");
            let thread: String = row.get("thread");
            let body: String = row.get("body");
            format!("
                <li>
                    <h3>{}/{} <small>{}</small></h3>
                    <p>{}</p>
                </li>
                ", sender, thread, timestamp, body)
        })
        .collect::<Vec<String>>()
        .join("");
    let markup = base(&format!("
        <h1>write-only.space</h1>
        <h2>latest posts</h2>
        <ul>
            {}
        </ul>
        ", posts));
    let resp = Response::with(
    ( "text/html".parse::<Mime>().unwrap()
    , status::Ok
    , markup
    ));
    Ok(resp)
}



// recipient   string  recipient of the message as reported by MAIL TO during SMTP chat.
// sender  string  sender of the message as reported by MAIL FROM during SMTP chat. Note: this value may differ from From MIME header.
// from    string  sender of the message as reported by From message header, for example “Bob <bob@example.com>”.
// subject     string  subject string.
// body-plain  string  text version of the email. This field is always present. If the incoming message only has HTML body, Mailgun will create a text representation for you.
// stripped-text   string  text version of the message without quoted parts and signature block (if found).
// stripped-signature  string  the signature block stripped from the plain text message (if found).
// body-html   string  HTML version of the message, if message was multipart. Note that all parts of the message will be posted, not just text/html. For instance if a message arrives with “foo” part it will be posted as “body-foo”.
// stripped-html   string  HTML version of the message, without quoted parts.
// attachment-count    int     how many attachments the message has.
// attachment-x    string  attached file (‘x’ stands for number of the attachment). Attachments are handled as file uploads, encoded as multipart/form-data.
// timestamp   int     number of seconds passed since January 1, 1970 (see securing webhooks).
// token   string  randomly generated string with length 50 (see securing webhooks).
// signature   string  string with hexadecimal digits generate by HMAC algorithm (see securing webhooks).
// message-headers     string  list of all MIME headers dumped to a json string (order of headers preserved).
// content-id-map  string  JSON-encoded dictionary which maps Content-ID (CID) of each attachment to the corresponding attachment-x parameter. This allows you to map posted attachments to tags like <img src='cid'> in the message body.


// https://documentation.mailgun.com/user_manual.html#parsed-messages-parameters
fn receive_email(req: &mut Request) -> IronResult<Response> {
    let data = req.get::<params::Params>().unwrap();
    let conn = req.get::<persistent::Read<PostgresDB>>().unwrap().get().unwrap();

    let sender = String::from_value(data.get("sender").unwrap()).unwrap();
    let thread = String::from_value(data.get("subject").unwrap()).unwrap();
    let body = String::from_value(data.get("stripped-html").unwrap()).unwrap();

    conn.execute("INSERT INTO post (sender, thread, body) VALUES ($1, $2, $3)",
        &[&sender, &thread, &body]).unwrap();

    let resp = Response::with(
    ( "text/html".parse::<Mime>().unwrap()
    , status::Ok
    , "wooo"
    ));
    Ok(resp)
}

// fn signup_form(req: &mut Request) -> IronResult<Response> {
//     let markup = base(&format!("
//         <form action=\"/signup\" method=\"POST\">
//             <div>
//                 <label for=\"email\">email address</lable>
//                 <input type=\"email\" id=\"email\" name=\"email\" placeholder=\"username@example.com\" />
//             </div>
//             <div>
//                 <button type=\"submit\">Sign up</button>
//             </div>
//         </form>
//         "));
//     let resp = Response::with(
//     ( "text/html".parse::<Mime>().unwrap()
//     , status::Ok
//     , markup
//     ));
//     Ok(resp)
// }

// fn handle_signup(req: &mut Request) -> IronResult<Response> {
//     let data = req.get_ref::<UrlEncodedBody>().unwrap();
//     println!("data {:?}", data);
//     let markup = base(&format!("
//         <p>wooo</p>"));
//     let resp = Response::with(
//     ( "text/html".parse::<Mime>().unwrap()
//     , status::Ok
//     , markup
//     ));
//     Ok(resp)
// }

fn env(name: &str, def: &str) -> String {
    std::env::var(name).unwrap_or(def.to_string())
}

fn get_pool(uri: &str) -> Result<PostgresPool, String> {
    let config = r2d2::Config::default();
    let manager = try!(PostgresConnectionManager::new(uri, SslMode::None)
        .map_err(|err| err.to_string()));
    r2d2::Pool::new(config, manager)
        .map_err(|err| err.to_string())
}


fn main() {
    let port = env("PORT", "").parse::<u16>().unwrap_or(8080);
    let dburl = env("DATABASE_URL", "postgresql://postgres@localhost");

    let (logger_before, logger_after) = Logger::new(None);

    let pool = get_pool(&dburl).unwrap();
    migrate::run(pool.get().unwrap()).unwrap();

    let mut router = Router::new();
    router.get("/", index, "index");
    router.post("/email", receive_email, "email");

    let mut chain = Chain::new(router);
    chain.link_before(logger_before);
    chain.link(PRead::<PostgresDB>::both(pool));
    chain.link_after(logger_after);

    match Iron::new(chain).http(("0.0.0.0", port)) {
      Ok(_) => println!("listening on {}...", port),
      Err(m) => println!("Failed to start on port {}: {}", port, m),
    }
}
