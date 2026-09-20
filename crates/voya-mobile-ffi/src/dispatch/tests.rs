use std::collections::BTreeSet;

use super::*;

/// Every command the backend registers, generated from `bindings.ts`.
const COMMAND_NAMES: &str = include_str!("../../../../packages/contracts/commands.json");

fn registered_commands() -> Vec<String> {
    serde_json::from_str(COMMAND_NAMES).expect("commands.json is a list of names")
}

/// The commands `route` answers, read off the source rather than listed twice.
///
/// A dispatcher is a `"name" =>` arm, so the match itself is the list; keeping
/// a second copy here would be the drift this test exists to catch.
fn dispatched_commands() -> BTreeSet<String> {
    include_str!("../dispatch.rs")
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix('"')?;
            let (name, tail) = rest.split_once('"')?;
            tail.trim_start()
                .starts_with("=>")
                .then(|| name.to_string())
        })
        .collect()
}

#[test]
fn every_registered_command_has_been_decided_about() {
    let dispatched = dispatched_commands();
    let declared: BTreeSet<&str> = UNSUPPORTED_ON_MOBILE
        .iter()
        .chain(NOT_YET_DISPATCHED)
        .copied()
        .collect();

    let undecided: Vec<String> = registered_commands()
        .into_iter()
        .filter(|command| !dispatched.contains(command) && !declared.contains(command.as_str()))
        .collect();

    assert!(
        undecided.is_empty(),
        "these backend commands have no dispatcher and are on neither list — \
         decide for each one whether a phone can ever answer it, and say so:\n  {}",
        undecided.join("\n  ")
    );
}

#[test]
fn a_command_is_in_exactly_one_of_the_three_states() {
    let dispatched = dispatched_commands();
    let unsupported: BTreeSet<&str> = UNSUPPORTED_ON_MOBILE.iter().copied().collect();
    let pending: BTreeSet<&str> = NOT_YET_DISPATCHED.iter().copied().collect();

    let contradictory: Vec<&str> = unsupported
        .iter()
        .chain(&pending)
        .copied()
        .filter(|command| dispatched.contains(*command))
        .collect();
    assert!(
        contradictory.is_empty(),
        "answered and declared unanswerable at once: {contradictory:?}"
    );

    let both: Vec<&&str> = unsupported.intersection(&pending).collect();
    assert!(
        both.is_empty(),
        "a command cannot be both permanently unsupported and merely pending: {both:?}"
    );
}

#[test]
fn neither_list_names_a_command_that_does_not_exist() {
    let registered: BTreeSet<String> = registered_commands().into_iter().collect();

    let unknown: Vec<&str> = UNSUPPORTED_ON_MOBILE
        .iter()
        .chain(NOT_YET_DISPATCHED)
        .copied()
        .filter(|command| !registered.contains(*command))
        .collect();

    // A command retired in Rust leaves a stale entry behind, which would then
    // quietly keep covering nothing.
    assert!(
        unknown.is_empty(),
        "these entries name commands the backend does not register: {unknown:?}"
    );
}

#[test]
fn neither_list_repeats_itself() {
    for (name, list) in [
        ("UNSUPPORTED_ON_MOBILE", UNSUPPORTED_ON_MOBILE),
        ("NOT_YET_DISPATCHED", NOT_YET_DISPATCHED),
    ] {
        let unique: BTreeSet<&str> = list.iter().copied().collect();
        assert_eq!(unique.len(), list.len(), "{name} repeats an entry");
    }
}

#[test]
fn the_dispatcher_really_reads_its_own_match_arms() {
    // The two tests above are only as good as this parse; a `route` that stops
    // matching on string literals would make them vacuously pass.
    let dispatched = dispatched_commands();

    assert!(dispatched.contains("runtime_status"), "{dispatched:?}");
    assert!(
        dispatched.contains("list_profile_summaries"),
        "{dispatched:?}"
    );
    assert!(
        dispatched.len() > 10,
        "only {} arms parsed",
        dispatched.len()
    );
}
