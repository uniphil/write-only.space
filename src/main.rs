extern crate chrono;
extern crate crypto;
extern crate iron;
extern crate logger;
extern crate params;
extern crate persistent;
extern crate postgres;
extern crate r2d2;
extern crate r2d2_postgres;
#[macro_use]
extern crate route;
extern crate url;

use iron::{Iron, Chain, Request, Response, IronResult, Plugin, status};
use iron::mime::Mime;
use iron::typemap::Key;
use logger::Logger;
use params::{FromValue};
use persistent::Read as PRead;
use r2d2_postgres::{SslMode, PostgresConnectionManager};
use url::percent_encoding::{PATH_SEGMENT_ENCODE_SET, utf8_percent_encode, percent_decode};

mod db;
mod migrate;

type PostgresPool = r2d2::Pool<PostgresConnectionManager>;

struct PostgresDB;
impl Key for PostgresDB {
    type Value = PostgresPool;
}


#[derive(Debug, PartialEq, Eq)]
enum Page<'a> {
    Home,
    Author(&'a str),
    Topic(&'a str, &'a str),
    ReceiveEmail,
    NotFound,
}


fn base(content: &str) -> String {
    format!("<!doctype html>
        <html>
            <head>
                <title>write-only.space</title>
            </head>
            <body>
                <h1>
                    <a href=\"/\" title=\"All authors\">write-only.space</a>
                </h1>
                {}
            </body>
        </html>
        ", content)
}


fn index(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<persistent::Read<PostgresDB>>().unwrap().get().unwrap();
    let authors = conn
        .query("
            SELECT
                sender,
                max(timestamp) as latest
            FROM post
            GROUP BY sender
            ORDER BY latest DESC", &[])
        .unwrap()
        .iter()
        .map(|row| {
            let sender: String = row.get("sender");
            format!("
                <li>
                    <a href=\"/{}\" title=\"Notes from {}\">{}</a>
                </li>
                ", utf8_percent_encode(&sender, PATH_SEGMENT_ENCODE_SET), sender, sender)
        })
        .collect::<Vec<String>>()
        .join("");
    let markup = base(&format!("
        <h2>authors</h2>
        <ul>
            {}
        </ul>
        ", authors));
    Ok(Response::with(
    ( "text/html".parse::<Mime>().unwrap()
    , status::Ok
    , markup
    )))
}


fn threads(req: &mut Request, email: &str) -> IronResult<Response> {
    let conn = req.get::<persistent::Read<PostgresDB>>().unwrap().get().unwrap();
    let ref author_email = percent_decode(email.as_bytes())
        .decode_utf8_lossy()
        .into_owned();
    let threads = conn
        .query("
            SELECT
                thread,
                max(timestamp) as latest
            FROM post
            WHERE sender = $1
            GROUP BY thread
            ORDER BY latest DESC
        ", &[author_email])
        .unwrap()
        .iter()
        .map(|row| {
            let topic: String = row.get("thread");
            format!("
                <li>
                    <a href=\"/{}/{}\" title=\"Notes in {}\">{}</a>
                </li>
                ", utf8_percent_encode(author_email, PATH_SEGMENT_ENCODE_SET), utf8_percent_encode(&topic, PATH_SEGMENT_ENCODE_SET), topic, topic)
        })
        .collect::<Vec<String>>();
    if threads.len() > 0 {
        let markup = base(&format!("
            <h2>Notes by {}</h2>
            <ul>
                {}
            </ul>
            ", author_email, threads.join("")));
        Ok(Response::with(
        ( "text/html".parse::<Mime>().unwrap()
        , status::Ok
        , markup
        )))
    } else {
        let markup = base(&format!("
            <h2>No notes by {}</h2>
            <p>Create notes by emailing <a href=\"mailto:note@write-only.space\">note@write-only.space</a> if {} is your email address.</p>
            <p>Notes are grouped into threads by the email subject.</p>
            ", author_email, author_email));
        Ok(Response::with(
        ( "text/html".parse::<Mime>().unwrap()
        , status::NotFound
        , markup
        )))
    }
}

fn notes(req: &mut Request, email: &str, topic: &str) -> IronResult<Response> {
    let conn = req.get::<persistent::Read<PostgresDB>>().unwrap().get().unwrap();
    let ref author_email = percent_decode(email.as_bytes())
        .decode_utf8_lossy()
        .into_owned();
    let ref topic = percent_decode(topic.as_bytes())
        .decode_utf8_lossy()
        .into_owned();

    let notes = conn
        .query("
            SELECT body, timestamp
            FROM post
            WHERE sender = $1 AND thread = $2
            ORDER BY timestamp DESC
        ", &[author_email, topic])
        .unwrap()
        .iter()
        .map(|row| {
            let body: String = row.get("body");
            let timestamp: chrono::NaiveDateTime = row.get("timestamp");
            format!("
                <li>
                    <p><strong>{}</strong></p>
                    {}
                </li>
                ", &timestamp.format("%Y %B %e"), body)
        })
        .collect::<Vec<String>>()
        .join("");
    let markup = base(&format!("
        <h2>
            <a href=\"/{}\" title=\"Notes by {}\">{}</a>
            &ndash;
            {}
        </h2>
        <ul>
            {}
        </ul>
        ", utf8_percent_encode(author_email, PATH_SEGMENT_ENCODE_SET), author_email, author_email, topic, notes));
    Ok(Response::with(
    ( "text/html".parse::<Mime>().unwrap()
    , status::Ok
    , markup
    )))
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


route_fn!(route -> Page {
    (/) => Page::Home,
    (/"email") => Page::ReceiveEmail,
    (/[email]) => Page::Author(email),
    (/[email]/[topic]) => Page::Topic(email, topic),
}, Page::NotFound);


fn router(req: &mut Request) -> IronResult<Response> {
    let path = format!("/{}", req.url.path().join("/"));
    match route(&path) {
        Page::Home => index(req),
        Page::Author(email) => threads(req, email),
        Page::Topic(email, subject) => notes(req, email, subject),
        Page::ReceiveEmail => receive_email(req),
        Page::NotFound =>
            Ok(Response::with((status::NotFound))),
    }
}


fn main() {
    let port = env("PORT", "").parse::<u16>().unwrap_or(8080);
    let dburl = env("DATABASE_URL", "postgresql://postgres@localhost");

    let (logger_before, logger_after) = Logger::new(None);

    let pool = get_pool(&dburl).unwrap();
    migrate::run(pool.get().unwrap()).unwrap();

    let mut chain = Chain::new(router);
    chain.link_before(logger_before);
    chain.link(PRead::<PostgresDB>::both(pool));
    chain.link_after(logger_after);

    match Iron::new(chain).http(("0.0.0.0", port)) {
      Ok(_) => println!("listening on {}...", port),
      Err(m) => println!("Failed to start on port {}: {}", port, m),
    }
}
