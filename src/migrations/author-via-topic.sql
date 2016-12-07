ALTER TABLE topic
    ADD COLUMN author citext REFERENCES author(email);


UPDATE topic
    SET author = post.author
    FROM post
    WHERE post.topic = topic.id;


ALTER TABLE post
    DROP COLUMN author;


ALTER TABLE topic
    ALTER COLUMN author
        SET NOT NULL,
    ADD FOREIGN KEY(author)
        REFERENCES author(email)
        ON UPDATE CASCADE ON DELETE CASCADE;
