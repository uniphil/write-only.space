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

use chrono::{DateTime, UTC, offset, NaiveDateTime, Duration};
use iron::{Iron, Chain, Request, Response, IronResult, Plugin};
use iron::status::Status;
use iron::mime::Mime;
use iron::typemap::Key;
use logger::Logger;
use params::{FromValue};
use persistent::Read as PRead;
use r2d2_postgres::{SslMode, PostgresConnectionManager};
use url::percent_encoding::{PATH_SEGMENT_ENCODE_SET, utf8_percent_encode, percent_decode};

mod db;
#[macro_use]
mod html;
mod migrate;

type PostgresPool = r2d2::Pool<PostgresConnectionManager>;

struct PostgresDB;
impl Key for PostgresDB {
    type Value = PostgresPool;
}

#[derive(Debug, PartialEq, Eq)]
struct Post {
    body: String,
    timestamp: NaiveDateTime,
}


#[derive(Debug, PartialEq, Eq)]
enum Title {
    Nothing,
    Add(String),
    Replace(String),
}
impl Title {
    fn add(self, child: Title, sep: &str) -> Title {
        use self::Title::*;
        match (self, child) {
            (Nothing,    Nothing)    => Nothing,
            (Nothing,    _child)     => _child,
            (_self,      Nothing)    => _self,
            (Add(s),     Add(c))     => Add(format!("{} {} {}", c, sep, s)),
            (Add(_),     Replace(c)) => Add(c),
            (Replace(s), Add(c))     => Replace(format!("{} {} {}", c, sep, s)),
            (Replace(_), Replace(c)) => Replace(c),
        }
    }
}
impl std::fmt::Display for Title {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use self::Title::*;
        write!(f, "🌘 {}", match *self {
            Nothing => "",
            Add(ref s) | Replace(ref s) => s,
        })
    }
}


#[derive(Debug, PartialEq, Eq)]
enum PageContent {
    Home { authors: Vec<(String, NaiveDateTime)> },
    Topics { author: String, topics: Vec<String> },
    Posts { author: String, topic: String, posts: Vec<Post> },
}


fn ul<I, T, F>(items: I, format_item: F) -> String
where I: IntoIterator<Item=T>,
      F: Fn(&T) -> String {
    tag!(ul: items
        .into_iter()
        .map(|item| tag!(li: format_item(&item)))
        .collect::<Vec<String>>()
        .join(""))
}

fn link_author(username: &String) -> String {
    let link = utf8_percent_encode(&username, PATH_SEGMENT_ENCODE_SET);
    let title = format!("Notes from {}", &username);
    tag!(a[href=link][title=title]: username)
}

fn days_ago(t: &NaiveDateTime) -> String {
    let now = UTC::now();
    let when: DateTime<UTC> = DateTime::from_utc(*t, offset::utc::UTC);
    match (when - now).num_days() {
        n if n > 1 => format!("in {} days", n),
        1          => format!("tomorrow"),
        0          => format!("today"),
        -1         => format!("yesterday"),
        n          => format!("{} days ago", n),
    }
}

fn link_author_latest(&(ref author, ref latest): &(String, NaiveDateTime)) -> String {
    tag!(p: link_author(&author), " ", days_ago(latest))
}

fn link_topic(&(username, ref topic): &(&String, String)) -> String {
    let link = format!("/{}/{}",
        utf8_percent_encode(username, PATH_SEGMENT_ENCODE_SET),
        utf8_percent_encode(&topic, PATH_SEGMENT_ENCODE_SET));
    let title = format!("Notes in {}", topic);
    tag!(a[href=link][title=title]: topic)
}

fn show_post(post: &Post) -> String {
    join![
        tag!(p: tag!(string: &post.timestamp.format("%Y %B %e"))),
        post.body]
}

fn home_page(authors: Vec<(String, NaiveDateTime)>) -> (Title, Status, String) {
    let title = String::from("Write like nobody's reading on write-only.space");
    (Title::Replace(title), Status::Ok, join![
        tag!(h1: "Write like nobody's reading"),
        tag!(p: "write-only is a tiny island in cyberspace where no one visits. Send an email to ",
            tag!(a[href="mailto:note@write-only.space"]: "note@write-only.space"),
            " and it will show up here, and no one will read it."),
        tag!(h2: "Please respect the authors"),
        tag!(p: "What you'll find are collections of unpolished thoughts, written without the pressures of an audience or Internet Points. It's not for sharing."),
        tag!(p: "So avoid linking to notes, especially on aggregation sites like reddit. If you're not sure, contact the author first."),
        tag!(h2: "Latest notes:"),
        ul(authors, &link_author_latest)
    ])
}

fn topics_page(author: String, topics: Vec<String>) -> (Title, Status, String) {
    if topics.len() > 0 {
        let ts = topics.into_iter().map(|t| (&author, t));
        (Title::Add((&author).to_string()), Status::Ok, join!(
            tag!(h2: "Notes by ", author),
            ul(ts, &link_topic)))
    } else {
        (Title::Nothing, Status::NotFound, join!(
            tag!(h2: "No notes by ", author),
            tag!(p: "Create notes by emailing ",
                tag!(a[href="mailto:note@write-only.space"]: "note@write-only.space"),
                " if ", author, " is your email address."),
            tag!(p: "Notes are grouped into threads by the email subject.")))
    }
}

fn posts_page(author: String, topic: String, posts: Vec<Post>) -> (Title, Status, String) {
    if posts.len() > 0 {
        let link = format!("/{}", utf8_percent_encode(&author, PATH_SEGMENT_ENCODE_SET));
        let title = join!("Notes by ", &author);
        (Title::Add((&topic).to_string()), Status::Ok, join!(
            tag!(h2:
                tag!(a[href=link][title=title]: author),
                " &ndash; ",
                topic),
            ul(posts, &show_post)))
    } else {
        let mailto = format!("mailto:note@write-only.space?subject={}",
            utf8_percent_encode(&topic, PATH_SEGMENT_ENCODE_SET));
        (Title::Nothing, Status::NotFound, join!(
            tag!(h2: "No notes on ", &topic, " by ", &author),
            tag!(p: tag!(strong: "Are you ", &author, "?")),
            tag!(p: "Post notes here by emailing them to ",
                tag!(a[href=mailto]: "note@write-only.space"),
                " with ", tag!(strong: &topic),  " as the subject line.")))
    }
}

fn render(page: PageContent) -> IronResult<Response> {
    let (title, status, content) = match page {
        PageContent::Home { authors } =>
            home_page(authors),
        PageContent::Topics { author, topics } =>
            topics_page(author, topics),
        PageContent::Posts { author, topic, posts } =>
            posts_page(author, topic, posts),
    };

    let html = {
        let title = Title::Add("write-only☄space".to_string()).add(title, "|");
        let style = include_str!("style.css");
        join!["<!doctype html>",
            tag!(html:
                tag!(head:
                    tag!(meta[charset="utf-8"]),
                    tag!(title: title),
                    tag!(style: style)
                ),
                tag!(body:
                    tag!(header:
                        tag!(a[href="/"][title="Home"]: "write-only☄space")
                    ),
                    tag!(section: content)
                )
            )]
    };

    Ok(Response::with(
    ( "text/html".parse::<Mime>().unwrap()
    , status
    , html
    )))
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
        .into_iter()
        .map(|row| (row.get("sender"), row.get("latest")))
        .collect();

    render(PageContent::Home { authors: authors })
}


fn threads(req: &mut Request, email: &str) -> IronResult<Response> {
    let conn = req.get::<persistent::Read<PostgresDB>>().unwrap().get().unwrap();
    let author = percent_decode(email.as_bytes())
        .decode_utf8_lossy()
        .into_owned();
    let topics = conn
        .query("
            SELECT
                thread,
                max(timestamp) as latest
            FROM post
            WHERE sender = $1
            GROUP BY thread
            ORDER BY latest DESC
        ", &[&author])
        .unwrap()
        .into_iter()
        .map(|row| row.get("thread"))
        .collect::<Vec<String>>();

    render(PageContent::Topics { author: author, topics: topics })
}

fn notes(req: &mut Request, email: &str, topic: &str) -> IronResult<Response> {
    let conn = req.get::<persistent::Read<PostgresDB>>().unwrap().get().unwrap();
    let author_email = percent_decode(email.as_bytes())
        .decode_utf8_lossy()
        .into_owned();
    let topic = percent_decode(topic.as_bytes())
        .decode_utf8_lossy()
        .into_owned();

    let posts = conn
        .query("
            SELECT body, timestamp
            FROM post
            WHERE sender = $1 AND thread = $2
            ORDER BY timestamp DESC
        ", &[&author_email, &topic])
        .unwrap()
        .into_iter()
        .map(|row| Post { body: row.get("body"), timestamp: row.get("timestamp") })
        .collect();

    render(PageContent::Posts { author: author_email, topic: topic, posts: posts })
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
    , Status::Ok
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


fn router(req: &mut Request) -> IronResult<Response> {
    let path = format!("/{}", req.url.path().join("/"));
    route!(path, {
    (/)                 => index(req);
    (/"email")          => receive_email(req);
    (/[email])          => threads(req, email);
    (/[email]/[topic])  => notes(req, email, topic);
    });
    Ok(Response::with((Status::NotFound)))
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
