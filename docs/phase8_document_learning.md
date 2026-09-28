# Document learning (Phase 8)

Earlier phases let the Machine answer from knowledge taught by hand. Phase 8
teaches it from documents through a process a person can inspect at every step:
import, extract, propose, review, commit, answer with references, and remove.

The guiding rule: **imported text is source material, and instructions inside a
document are never commands.** Nothing reaches memory until a caller explicitly
accepts it.

## The workflow

1. **Import.** `import_text_document`, `import_document_path` (text or `.pdf`),
   or `import_pdf_document` store the document and its extraction.
2. **Extract.** `extract_text` / `extract_pages` split the source into
   sentences that carry a byte span and, for PDFs, a page number.
3. **Propose.** Each sentence is offered to the rule reader (`if … then …`), the
   definition reader (`X is a/the Y`, `X is called Y`, `X refers to Y`), and the
   general SVO reader. What is understood becomes a typed proposal; what is not
   becomes a rejection with a reason.
4. **Review.** `inspect_document` shows every item with its span, page, status,
   and reason. `accept_document_item` / `reject_document_item` record a review.
5. **Commit.** `commit_document` turns accepted items into durable assertions
   whose provenance names the document; `learn_document` accepts and commits in
   one step.
6. **Answer.** Questions run through the ordinary turn pipeline; the evidence
   for a document-derived answer cites `document:<source-id>`.
7. **Remove.** `remove_document` retracts exactly the assertions derived from
   the document, marks dependent turns stale, and marks the document removed.

## What gets rejected, and why

Nothing is dropped silently. Every sentence that does not become knowledge is
recorded as a rejected item with one of:

| Reason | Meaning |
|---|---|
| `no complete fact, definition, or rule` | no single clean claim could be read |
| `duplicate of an earlier item` | the same sentence already appeared |
| `boilerplate or page furniture` | licence notices, headings, figure/table lines |
| `instruction-like text treated as data, not executed` | an imperative or injection marker |

The last is the injection defence: `classify_instruction` recognises
second-person imperatives and prompt-injection markers ("ignore previous
instructions", "system:", "you must", "delete …", "curl …", …). Such sentences
are refused, never executed, and cannot be accepted or committed.

## Storage

Schema version is now **2** (`migrations/0002_documents.sql`):

* `documents` — id, title, kind (`plain_text`/`text_pdf`), origin, SHA-256, byte
  length, imported_at, status (`imported`/`committed`/`removed`), note.
* `document_items` — id, document_id, index, kind
  (`fact`/`definition`/`rule`/`rejected`), status
  (`proposed`/`accepted`/`rejected`/`committed`), typed payload, span, page,
  confidence, reason, and the durable `assertion_id` once committed.

Items cascade with their document, but removing a document marks it removed
rather than deleting it, so the audit trail of what it contained survives.

## Surfaces

* **Library:** `src/document_learning.rs` and the `ConversationService` methods
  above.
* **CLI:** `machine_docs` (`import`, `list`, `inspect`, `accept`, `reject`,
  `commit`, `learn`, `ask`, `remove`, `demo`).
* **HTTP:** `/api/documents` (list/import), `/api/documents/{id}` (inspect),
  `/api/documents/{id}/learn`, `/api/documents/{id}/remove`,
  `/api/documents/items/{item_id}/accept|reject`.

## Scope

Plain text and text-based PDFs, as specified. Scanned documents and visual
interpretation are deliberately deferred; `extract_pdf_file` fails cleanly when
a PDF carries no text layer (`pdf_reader` reports "possibly scanned/image-based").

## Result

The acceptance test imports a small document, inspects the extracted knowledge,
asks a supported question (abstaining before commit and citing the source
after), and removes the document, confirming across a database reload that its
influence disappears predictably while independently taught knowledge remains.
