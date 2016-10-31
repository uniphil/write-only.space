use hyper::Client;
use hyper::header::{Authorization, Basic, Connection, ContentType};
use hyper::mime::{Mime, TopLevel, SubLevel};
use hyper::net::HttpsConnector;
use hyper_rustls;
use url::percent_encoding::{PATH_SEGMENT_ENCODE_SET, QUERY_ENCODE_SET, utf8_percent_encode};


pub fn welcome(domain: &str, api_key: &str, to: &str, topic: &str, message_id: Option<&str>) {
    let from = "write-only <note@write-only.space>";
    let html = {
        let title = "Welcome to write-only 🌘";
        let header = tag!(td[style="padding: 1.5em 1em 1em 1em; text-align: center; font-size: 18px"][bgcolor="#000000"]:
            tag!(a[href="http://write-only.space"][style="color: #ffff00; text-decoration:none"]: "write-only☄space"));
        let post_link = format!("{}/{}/{}",
            utf8_percent_encode(domain, PATH_SEGMENT_ENCODE_SET),
            utf8_percent_encode(to, PATH_SEGMENT_ENCODE_SET),
            utf8_percent_encode(topic, PATH_SEGMENT_ENCODE_SET));
        let main = tag!(td[bgcolor="#003344"][style="color: #ffffff; padding: 1em 1em 1em 1em; font-size: 18px"]:
            tag!(b[style="font-size: 24px; padding: 1em 0 1em 0;"]: title),
            tag!(p:
                "You just posted your first note about ",
                tag!(a[href=post_link][style="font-weight: bold; color: #ffff00; text-decoration:none"]: topic),
                " – awesome!"),
            tag!(p:
                "We group notes by the email's subject, so you can post more about ",
                tag!(i: topic),
                " by simply replying to this email."),
            tag!(p: "That's it!"),
            tag!(p: "✎ Happy writing"));
        join![
            "<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.0 Transitional//EN\" \"http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd\">",
            tag!(html[xmlns="http://www.w3.org/1999/xhtml"]:
                tag!(head:
                    "<meta http-equiv=\"Content-Type\" content=\"text/html; charset=UTF-8\" />",
                    tag!(title: title),
                    tag!(meta[name="viewport"][content="width=device-width, initial-scale=1.0"])),
                tag!(body[style="margin: 0; padding: 0;"]:
                    tag!(table[border="0"][cellpadding="0"][cellspacing="0"][width="400"]:
                        tag!(tr: header),
                        tag!(tr: main))))
        ]
    };
    let mut payload = format!("from={}&to={}&subject={}&html={}&o:tag=welcome",
        utf8_percent_encode(&from, QUERY_ENCODE_SET),
        utf8_percent_encode(&to, QUERY_ENCODE_SET),
        utf8_percent_encode(&topic, QUERY_ENCODE_SET),
        utf8_percent_encode(&html, QUERY_ENCODE_SET));
    if let Some(mid) = message_id {
        payload.push_str(&format!("&h:In-Reply-To={id}&h:References={id}",
            id = utf8_percent_encode(&mid, QUERY_ENCODE_SET)));
    }
    let response = Client::with_connector(HttpsConnector::new(hyper_rustls::TlsClient::new()))
        .post(&format!("https://api.mailgun.net/v3/{}/messages", domain))
        .header(Authorization(Basic {
            username: "api".to_owned(),
            password: Some(api_key.to_owned())
        }))
        .header(ContentType(Mime(TopLevel::Application, SubLevel::WwwFormUrlEncoded, vec![])))
        .header(Connection::close())
        .body(&payload)
        .send();
    if let Ok(r) = response {
        if r.status.is_success() {
            println!("sent welcome email to {}", to);
        } else {
            println!("failed to send welcome email to {}: {}", to, r.status);
        }
    } else {
        println!("failed to send welcome email to {}", to);
    };
}
