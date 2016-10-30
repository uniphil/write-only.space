macro_rules! join {
    () => (
        String::from("")
    );
    ($s:expr $(, $rest:expr)*) => (
        format!("{}{}", $s, join!($($rest),*))
    );
}

macro_rules! join_with {
    ($j:expr) => (
        String::from("")
    );
    ($j:expr; $s:expr) => (
        format!("{}", $s)
    );
    ($j:expr; $s:expr, $($rest:expr),+) => (
        format!("{}{}{}", $s, $j, join_with!($j; $($rest),*))
    );
}

macro_rules! tag {
    ($n:ident) => (
        format!("<{} />",
            stringify!($n))
    );
    ($n:ident $([$p:ident=$v:tt])*) => (
        format!("<{} {} />",
            stringify!($n),
            join_with![" "; $(format!("{}=\"{}\"", stringify!($p), $v)),*])
    );
    ($n:ident: $($c:expr),*) => (
        format!("<{n}>{c}</{n}>",
            n=stringify!($n),
            c=join![$($c),*])
    );
    ($n:ident $([$p:ident=$v:tt])*: $($c:expr),*) => (
        format!("<{n} {a}>{c}</{n}>",
            n=stringify!($n),
            a=join_with![" "; $(format!("{}=\"{}\"", stringify!($p), $v)),*],
            c=join![$($c),*])
    );
}


#[test]
fn test_tag() {
    assert_eq!(&tag!(br), "<br />");
    assert_eq!(&tag!(link[rel="stylesheet"][href="/style.css"]), "<link rel=\"stylesheet\" href=\"/style.css\" />");
    assert_eq!(&tag!(p: "hello", "world"), "<p>helloworld</p>");
    assert_eq!(&tag!(button[type="submit"]: "go"), "<button type=\"submit\">go</button>");
}
