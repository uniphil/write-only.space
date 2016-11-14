CREATE TABLE topic
(   id          uuid PRIMARY KEY DEFAULT uuid_generate_v4()
,   timestamp   timestamp NOT NULL DEFAULT now()
,   topic       citext NOT NULL
,   key         uuid NOT NULL DEFAULT uuid_generate_v4()
);


INSERT INTO topic (topic, timestamp)
    SELECT
        thread,
        min(post.timestamp)  -- first
    FROM post
    GROUP BY thread;


ALTER TABLE post
    ADD COLUMN topic uuid REFERENCES topic(id);


UPDATE post
    SET topic = topic.id
    FROM topic
    WHERE post.thread = topic.topic;


ALTER TABLE post
    ALTER COLUMN topic SET NOT NULL,
    DROP COLUMN thread;
