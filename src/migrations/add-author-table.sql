CREATE TABLE author
( email     citext PRIMARY KEY NOT NULL
    CONSTRAINT could_be_valid_email CHECK (
      length(email) <= 254
      AND email like '%_@_%'
    )
, timestamp timestamp NOT NULL DEFAULT now()
, username  text UNIQUE
);

-- give existing authors each a row
INSERT INTO author
    (email, timestamp)
SELECT
    sender as email,
    min(timestamp) as timestamp
FROM post
GROUP BY sender;

-- remove post's email address validation
ALTER TABLE post
DROP CONSTRAINT could_be_valid_email;

-- better name for the soon-to-be FK
ALTER TABLE post
RENAME COLUMN sender TO author;

-- make author an FK
ALTER TABLE post
ADD FOREIGN KEY(author)
REFERENCES author(email)
ON UPDATE CASCADE ON DELETE CASCADE;
