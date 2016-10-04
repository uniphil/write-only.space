CREATE EXTENSION "uuid-ossp";
CREATE EXTENSION citext;

CREATE TABLE post
( id            uuid PRIMARY KEY DEFAULT uuid_generate_v4()
, timestamp     timestamp DEFAULT now()
, sender        citext NOT NULL
    CONSTRAINT could_be_valid_email CHECK (
      length(sender) <= 254
      AND sender like '%_@_%'
    )
, thread        text NOT NULL
, body          text NOT NULL
);
