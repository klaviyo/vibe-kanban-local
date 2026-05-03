-- Synthesizes the `txid` field of `MutationResponse<T>` for the local server.
-- A mutating route inserts one row into this table inside the same transaction
-- as its data write; the returned id becomes the wire txid. AUTOINCREMENT
-- guarantees a strictly-increasing id over the lifetime of the database, and
-- the insert participates in the surrounding transaction so a rollback also
-- rolls back the id allocation -- a rolled-back data write never produces a
-- txid that any client observes.
CREATE TABLE mutation_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT
);
