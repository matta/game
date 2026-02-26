use core::journal::InputJournal;
use core::replay::replay_to_end;
use core::state::ContentPack;

#[test]
fn test_determinism_identical_seeds_produce_same_hash() {
    let content = ContentPack {};
    let seed1 = 12345;
    let mut journal1 = InputJournal::new(seed1);
    journal1.append_choice(core::ChoicePromptId(0), core::Choice::KeepLoot, 0);

    let seed2 = 12345;
    let mut journal2 = InputJournal::new(seed2);
    journal2.append_choice(core::ChoicePromptId(0), core::Choice::KeepLoot, 0);

    let result1 = replay_to_end(&content, &journal1).expect("Replay 1 failed");
    let result2 = replay_to_end(&content, &journal2).expect("Replay 2 failed");

    assert_eq!(
        result1.final_snapshot_hash, result2.final_snapshot_hash,
        "Identical runs must produce identical hashes"
    );
    assert_eq!(result1.final_tick, result2.final_tick);
}

#[test]
fn test_determinism_different_seeds_produce_different_hashes() {
    let content = ContentPack {};
    let mut journal1 = InputJournal::new(123);
    journal1.append_choice(core::ChoicePromptId(0), core::Choice::KeepLoot, 0);

    let mut journal2 = InputJournal::new(456);
    journal2.append_choice(core::ChoicePromptId(0), core::Choice::KeepLoot, 0);

    let result1 = replay_to_end(&content, &journal1).expect("Replay 1 failed");
    let result2 = replay_to_end(&content, &journal2).expect("Replay 2 failed");

    assert_ne!(
        result1.final_snapshot_hash, result2.final_snapshot_hash,
        "Different seeds should probably produce different outcomes or hashes"
    );
}
