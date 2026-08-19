## What this changes

<!-- One or two sentences. Explain why, not just what. -->

## Did any score move?

<!--
If you changed check behaviour, re-run the acceptance fixtures and list the repositories
whose scores changed, with the reason:

    ./scripts/fetch-fixtures.sh
    for d in target/fixtures/*/; do cargo run -q -- check "$d"; done

A silent score change is the single thing reviewers most need to see. Write "no behaviour
change" if that is the case.
-->

## Checklist

- [ ] `cargo test` passes
- [ ] `cargo clippy --all-targets` is clean
- [ ] Every new deduction ships an actionable `Fix` with file-level evidence
- [ ] New judgement calls have a regression test named after the repository that motivated them
- [ ] Docs updated if behaviour or scoring or the scoring contract changed
