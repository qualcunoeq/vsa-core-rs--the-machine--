# Third-Party Notices

The Machine contains or references third-party datasets, source texts, and benchmark material. These are not licensed under [The Machine Research Source License 1.0](LICENSE). They remain under their own terms, and those terms govern your use of that material. The license excludes this material in its Third-Party Materials section.

The identifiers, citations, and license strings below are the ones recorded in the repository metadata and in the source files themselves. The authoritative terms are always the upstream terms. Confirm the current upstream license before redistributing any of this material.

If you believe material is attributed incorrectly or is included without a valid license, contact <gliracurcio@gmail.com>.

## Datasets

### GSM8K

Files: `data/third_party_gsm8k_release_v1.json`, `data/third_party_gsm8k_restricted_release_v1.json`, `data/third_party_gsm8k_restricted_release_v2.json`, `data/third_party_gsm8k_prompt_subset_v1.jsonl`, `data/third_party_gsm8k_restricted_prompt_subset_v1.jsonl`, `data/third_party_gsm8k_restricted_prompt_subset_v2.jsonl`, and the derived quantity candidate and planner files.

- Source: OpenAI `grade-school-math` (GSM8K)
- Citation: Cobbe et al., Training Verifiers to Solve Math Word Problems (GSM8K), 2021
- Locator: `https://raw.githubusercontent.com/openai/grade-school-math/master/grade_school_math/data/test.jsonl`
- License recorded: MIT
- Retrieved: 2026-07-23

### MATH

Referenced in `docs/goal1_external_math_exam_release_manifest.json` and the external problem assemblies derived from it.

- Source: MATH dataset
- Citation: Dan Hendrycks et al., Measuring Mathematical Problem Solving With the MATH Dataset, NeurIPS 2021
- Locator: `https://huggingface.co/datasets/jeggers/competition_math`
- License recorded: MIT

### Humanity's Last Exam (HLE)

Referenced by dataset hash in the HLE audit reports (`docs/phase*_hle_*.md` and their reports). The raw question set is not checked in; the repository contains project-generated audit outputs and hash-only traces. Obtain the dataset under its own terms.

## Source texts

### OpenStax

Files: `docs/sources/openstax_*.txt` and `docs/sources/openstax_*.json`.

Books referenced: Precalculus 2e, Prealgebra 2e, Introductory Statistics 2e, Contemporary Mathematics, Principles of Economics 3e, Principles of Data Science, University Physics Volume 1 and Volume 2, Biology 2e, Chemistry 2e.

- Publisher: OpenStax, Rice University
- Locator: `https://openstax.org`
- License recorded: CC BY 4.0, attribution required
- One exception: `docs/sources/openstax_bayes_rule_catalog.txt` records CC BY-NC-SA 4.0 (Principles of Data Science).

### MIT OpenCourseWare

Files: `docs/sources/mit_analytic_number_theory_arithmetic_functions.txt` (18.781, Theory of Numbers), `docs/sources/mit_analytic_number_theory_character_definition.txt` (18.785, Analytic Number Theory), and an MIT OCW 6.045J reference in `docs/sources/bounded_finite_automata_source.txt`.

- Provider: MIT OpenCourseWare
- Locator: `https://ocw.mit.edu`
- License recorded: MIT OpenCourseWare, attribution required. MIT OpenCourseWare material is generally published under Creative Commons BY-NC-SA 4.0; confirm the current upstream terms.

### Topology Without Tears

Files: `docs/sources/topology_without_tears_*.txt`.

- Author: Sydney A. Morris
- Locator: `https://www.topologywithouttears.net`
- Terms recorded: author-provided freely available educational text. Confirm the current upstream terms.

## Fixtures

`data/third_party_corpus_fixture_v1.json` is a schema fixture, marked `not-evidence` in its own metadata. It contains no third-party content and is not covered by these notices.

## General

Where a file combines third-party material with original modifications, annotations, or arrangement added by the Licensor, the Licensor's original contribution is licensed under the research license, and the third-party material remains under its own terms.
