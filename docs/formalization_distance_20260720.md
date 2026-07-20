# Formalization-distance report

This report is diagnostic only. It does not authorize retrieval, theorem application, or solver execution.

Scanned **2500** questions. The benchmark flags **271** rows with textual visual references; the remaining attachment flags are metadata only and do not prove that visual reasoning is required.

## Modeling distance

| Distance | Questions |
|---|---:|
| direct_instantiation | 635 |
| executable_object | 271 |
| method_selection | 232 |
| one_modeling_step | 1037 |
| specialist_reasoning | 325 |

## Formalization status

| Status | Questions |
|---|---:|
| attachment_required | 271 |
| partially_structured | 1666 |
| structured | 563 |

## Input dependencies (orthogonal)

| Dependency | Questions |
|---|---:|
| diagram | 111 |
| graph | 73 |
| image | 271 |
| table | 132 |
| text_only | 2229 |

## Modeling obligations

| Obligation | Questions |
|---|---:|
| construct_equation | 264 |
| define_object | 965 |
| determine_target_semantics | 670 |
| establish_boundary_conditions | 45 |
| establish_initial_conditions | 138 |
| extract_quantifiers | 60 |
| identify_domain | 148 |
| parse_attachment | 271 |
| resolve_entity_reference | 323 |
| select_approximation_regime | 80 |
| select_specialized_method | 349 |

The runtime remains unchanged; this scan only identifies where formalization work is required.
