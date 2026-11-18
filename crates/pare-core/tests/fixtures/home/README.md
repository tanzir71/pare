# Rule fixtures

Copied into a temporary FakeHome by `tests/rules/main.rs`; nothing here is ever modified.

- A directory named `ENC~a~b` becomes Claude Code's encoding of `<home>/a/b` (`~` stands for a path separator).
- `{HOME}` inside `.jsonl` files becomes the fake home path (JSON-escaped).
