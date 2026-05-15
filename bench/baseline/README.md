# bench/baseline/

Persistent backup of the criterion `pre-rope` baseline JSONs (which normally
live under `target/criterion/<bench>/pre-rope/` and are wiped by
`cargo clean`).

## Restore after cargo clean

```bash
tar xzf bench/baseline/criterion-pre-rope.tar.gz -C target/criterion
```

After that, `cargo bench -- --baseline pre-rope` works again as if you had
just captured the baseline.

## Re-create

If you need to refresh the tarball:

```bash
mkdir -p bench/baseline
find target/criterion -name pre-rope -type d -print \
  | sed 's|^target/criterion/||' \
  > /tmp/pre-rope-paths.txt
( cd target/criterion && tar czf - -T /tmp/pre-rope-paths.txt ) \
  > bench/baseline/criterion-pre-rope.tar.gz
```
