# Expanded technical-language checkpoint

This shadow benchmark places independently authored, shifted technical text
in front of five source-derived curriculum frontends and their real
downstream evaluators:

* finite topology;
* bounded chemistry;
* bounded DNA biology;
* exact rectangular complex arithmetic;
* finite source-derived statistics.

Each route has 400 reports: 240 supported, 80 ambiguous, and 80 unsupported.
The reports contain reordered context, incidental formulas, multiple candidate
spans, missing orientation or target information, and explicit near-domain
requests. A complete frontend must emit a typed request with provenance before
the source-derived evaluator is invoked.

## Results

The machine-readable report is
[`stage_c_expanded_technical_language_2000.json`](stage_c_expanded_technical_language_2000.json)
with corpus hash
`08a1c9d8d7e360d69f99376a873b6d69a7fe65352ca218de04be7533826252fa`.

| Metric | Result |
|---|---:|
| Cases | 2,000 |
| Supported / ambiguous / unsupported | 1,200 / 400 / 400 |
| Complete downstream authorizations | 1,200 / 1,200 |
| Target-grounded reports | 2,000 / 2,000 |
| Ambiguities preserved | 400 / 400 |
| Unsupported requests refused | 400 / 400 |
| Frontend and downstream replay | 2,000 / 2,000 |
| Tamper rejections | 2,000 / 2,000 |
| Provenance preserved | 2,000 / 2,000 |
| False authorizations | 0 |
| False denials | 0 |

This is a bounded technical-language gate, not unrestricted natural-language
competence. It does not mutate the production registry, curriculum manifest,
or live routing.
