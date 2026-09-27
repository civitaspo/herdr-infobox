src/ingest.rs:L12: stdlib: The manual buffer and overflow loop duplicates bounded reading and draining. Use `input.by_ref().take(MAX_INPUT + 1).read_to_end(&mut bytes)?`, check the length, and drain oversized input with `std::io::copy(&mut input, &mut std::io::sink())?` before returning the truncation error; retain the existing deadline thread (13 lines fewer).
src/store.rs:L131: shrink: The transaction closure unwraps and immediately rewraps the same result. Replace it with `atomic(&mut self.conn, |tx| apply_batch(tx, batch))` (3 lines fewer).
net: -16 lines possible.
