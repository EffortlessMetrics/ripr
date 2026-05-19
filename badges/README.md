# Badge endpoints

This directory contains generated Shields endpoint JSON used by README badges.
The public `ripr` / `ripr+` endpoints are user-actionable repair counters, not
seam inventory. Detailed basis and inventory reports stay in `target/`.

Regenerate:

```bash
cargo xtask badges
```

Check drift:

```bash
cargo xtask badges --check
```

Only committed `*.json` endpoint files are public badge surfaces. Detailed
reports stay in CI artifacts and `target/`.
