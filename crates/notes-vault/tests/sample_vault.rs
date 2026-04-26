use notes_vault::{NoteBlock, Vault};
use std::path::PathBuf;

#[test]
fn loads_sample_vault() {
    let vault_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vault");
    let notes = Vault::new(vault_root).load_notes().unwrap();

    let image_count = notes.iter().map(|note| note.images.len()).sum::<usize>();
    let formula_count = notes
        .iter()
        .flat_map(|note| note.blocks.iter())
        .filter(|block| matches!(block, NoteBlock::Formula(_)))
        .count();

    assert_eq!(notes.len(), 15);
    assert_eq!(image_count, 5);
    assert!(formula_count > 40, "expected many formula blocks");
}
