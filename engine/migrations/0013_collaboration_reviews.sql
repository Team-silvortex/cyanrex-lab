-- Explicit C2-D staging only. Never install in an existing schema or on Engine startup.
CREATE TABLE collaboration_review_schema (
    singleton BOOLEAN PRIMARY KEY CHECK (singleton),
    version INTEGER NOT NULL,
    authority_id UUID NOT NULL,
    workspace_id UUID NOT NULL
);
CREATE TABLE collaboration_reviews (
    review_id UUID PRIMARY KEY,
    reviewer_id UUID NOT NULL,
    revision BIGINT NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991)
);
CREATE TABLE collaboration_review_revisions (
    review_id UUID NOT NULL REFERENCES collaboration_reviews(review_id),
    revision BIGINT NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991),
    snapshot TEXT NOT NULL CHECK (octet_length(snapshot) <= 65536),
    PRIMARY KEY (review_id, revision)
);
CREATE TABLE collaboration_review_outbox (
    event_id UUID PRIMARY KEY,
    review_id UUID NOT NULL,
    revision BIGINT NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991),
    actor_id UUID NOT NULL,
    event_type TEXT NOT NULL CHECK (event_type IN ('cyanrex.review.recorded', 'cyanrex.review.amended')),
    snapshot TEXT NOT NULL CHECK (octet_length(snapshot) <= 65536),
    UNIQUE (review_id, revision),
    FOREIGN KEY (review_id, revision) REFERENCES collaboration_review_revisions(review_id, revision)
);
