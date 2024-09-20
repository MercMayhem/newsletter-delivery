-- Your SQL goes here
CREATE TABLE newsletter_issues (
   newsletter_issue_id uuid NOT NULL,
   title TEXT NOT NULL,
   text TEXT NOT NULL,
   html TEXT NOT NULL,
   published_at TEXT NOT NULL,
   PRIMARY KEY(newsletter_issue_id)
);
