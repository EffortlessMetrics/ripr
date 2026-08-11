from pathlib import Path

path = Path("xtask/src/command.rs")
text = path.read_text(encoding="utf-8")
old = '''        command_entry(
            "source-promotion preflight --source-parent <sha> --swarm-parent <sha> --swarm-ref <immutable-ref> --source-repo <path> --swarm-repo <path> --version <version> [--resolved-tree <full-tree-sha>] [--swarm-main <rev>] [--source-main <rev>] [--out <dir>]",
            "external_state_read",
            "<out>/source-promotion-preflight.{json,md}",
            false,
            false,
            "Validates exact source/swarm parents, repository identity and reachability, deterministic ancestry counts/digests, and a disposable merge-tree conflict inventory; it never mutates either repository, constructs a join, changes versions, or publishes.",
        ),
'''
new = '''        command_entry(
            "source-promotion <preflight|verify> ...",
            "argument_dependent",
            "preflight or exact-J verification receipts under explicit --out or target/ripr/source-promotion",
            false,
            false,
            "Runs the read-only exact-pair preflight producer or the v2 exact history-preserving join verifier. Neither subcommand constructs a join, mutates Git refs, changes versions, or publishes.",
        ),
'''
count = text.count(old)
if count != 1:
    raise SystemExit(f"expected one swarm source-promotion catalog entry, found {count}")
text = text.replace(old, new)
path.write_text(text, encoding="utf-8", newline="\n")
