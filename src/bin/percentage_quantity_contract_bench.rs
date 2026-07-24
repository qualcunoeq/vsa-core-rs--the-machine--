use the_machine::percentage_quantity_proposal::corpus;

fn main() {
    let corpus = corpus();
    let errors = corpus.validation_errors();
    assert!(errors.is_empty(), "contract corpus errors: {errors:?}");
    let (supported, ambiguous, unsupported) = corpus.counts();
    println!(
        "percentage-quantity-contract: release={} hash={} cases={} supported={} ambiguous={} unsupported={} rewrite_pairs={} validation_errors={} deterministic=true",
        corpus.release_id,
        corpus.release_hash(),
        corpus.cases.len(),
        supported,
        ambiguous,
        unsupported,
        corpus.rewrite_pairs(),
        errors.len(),
    );
}
