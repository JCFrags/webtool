# Two-sprint plan

## Position and purpose

The research core through PR #24 is delivered from
`d961aeb0407607fa3e414b5ebafc08b6c6191fe0`. Published alpha assets are unchanged.
This plan starts new development, not another delivery of that core.

Make Webtool useful to other Linux users and container operators, without this
workstation's paths, configuration, helpers, or saved data. Keep one Rust service,
a thin text CLI, shared libraries, exact originals, and honest evidence limits.
Ordinary research remains LLM-free.

Sprint 1 is active. Sprint 2 and GitHub merges are blocked by the comparison gate.
No gate pass is claimed in this document.

## Sprint 1

Use parallel, scoped work on these areas:

| Area | Bounded first targets | Practical acceptance |
|---|---|---|
| Documents | Source-preserving CommonMark structure and styles, a verified small Office-format set using existing readers, and reproduced HTML/native-PDF fidelity corrections | Read, ingest, extract, and export exact originals. Retain source locations, warnings, zeros, empty values, and unavailable states. Distinguish tested formats from compiled support. |
| GitHub/code | Public issues/PRs/releases, pinned revision comparison, and deterministic selected-file declaration/context navigation | Exercise bounded unauthenticated operations. Retain first-party artifacts and revision/snapshot identities. Keep index snippets separate from source text and show pagination/coverage limits. |
| Backend | Typed failure categories, stable public codes, safe messages, and consistent single/batch HTTP errors | Producers select typed errors. Unknown faults remain internal errors. Do not classify failures by matching arbitrary error text. Preserve compatible response shapes and ordinary source behavior. |
| Linux/Docker | Reusable installation, clean-user onboarding, supported Linux candidate artifacts, and a usable persistent-data container path | Exercise isolated client/server installation and container use without personal configuration. Document requirements, checksums, source access, upgrades, and rollback. |

Do not add macOS or Windows work in this sprint. Do not claim all Linux
architectures or distributions from one successful host build. Dockerfile/OCI
validation with Podman is not proof that Docker Engine itself was exercised.

OCR, layout models, and other model-dependent document paths require separate
preparation decisions. Record those dependencies explicitly. Do not replace a
missing source value with generated or guessed text. Broader format and fidelity
work remains in the Documents area until it has measured support or an explicit
scope decision. A partial first slice is not completion of every document format.

## Sprint 1 comparison gate

The direct competitor is [Ketch](https://github.com/1broseidon/ketch).
Pin the inspected source and executable identities before running the comparison.
The initial competitor pin is current main
`3722f58c1ff5c6b14b687ca3f925587996033536`. Record its differences from released
`v0.18.1`. Use optimized builds for both tools.

### Inputs and fairness

1. Define the overlap matrix before scoring outputs. Include representative
   HTML/PDF reading and extraction, batch outcomes, search, code/docs, crawl/map,
   saved-source reuse, and MCP where the workflows genuinely overlap.
2. Run the same exact inputs through each real interface. Keep input hashes,
   source references, commands, limits, providers, transports, and cache states.
3. Separate repeatable retained/local-source cases from live provider observations.
   A blocked provider is not proof that another tool has better result quality.
   An unexercised material overlap keeps the gate open.
4. Compare equivalent cold and warm states. Report startup/setup separately, but
   do not hide the cost of a required daemon or helper.
5. Retain errors, failed inputs, partial results, warnings, and provenance. A
   nonempty output or successful process exit is not content acceptance.

Do not copy competitor bypasses, key rotation, arbitrary helper commands, or a
hidden extractor ladder merely to match a feature. Report any intentional boundary
that prevents an equivalent operation. Do not silently remove that case from the
gate.

### Measurements

Measure input-to-complete-output wall clock with repeated matched cases and
reported variation. Count the complete Webtool CLI, daemon, and helper footprint,
not only the thin client. Report peak and idle memory, CPU time, retained storage,
and relevant request/helper costs. Record measurement method and limits.

Webtool must meet or exceed Ketch quality and wall-clock performance in overlapping
workflows. Do not use a favorable overall average to hide a material regression.
Slightly higher system resources are allowed only when time and quality are at
parity or better. The numeric resource tolerance remains open and must be stated
before accepting that exception. The provisional 10% CPU/RAM target is not an
approved exception or a passing result.

### Quality

Use objective checks against retained originals for exact text, code, tables,
math notation, links, qualifications, source identity, and failure disclosure.
Then use a brand-new subagent for subjective evaluation. Randomize product-blinded
output labels. Give the evaluator source evidence and user tasks, not product
names, timings, implementation history, or the desired winner. Preserve the
label key separately and reveal it only after the evaluation is complete.

A gate receipt must identify exact builds, inputs, measurements, blind results,
remaining limits, and all material deficits. Fix reproduced deficits and rerun
only affected cases plus a bounded regression check. Do not lower the gate after
seeing the result.

## Sprint 2

Start only after the Sprint 1 gate passes:

- Optional cited answers over up to five fetched destination pages. Keep generated
  output separate, protect credentials, expose failed reads, and bound cancellation
  and resources. Endpoint/model selection and actual calls retain separate decisions.
- Scholarly and bibliography extensions, including additional structured-paper
  routes, historical selection, citation graphs, and reference-manager integration.
- Additional provider-specific search interfaces and filters.
- Media follow-ons such as playlists, compatible resume, and optional speech
  recognition. Source permissions, live transfers, and allocations remain separate.

Passing the gate does not grant account access, paid spending, model preparation,
live media permission, or a wider network trust boundary.

## Integration and preservation

Keep Sprint 1 work in focused branches/worktrees until the gate passes. Preserve
the running installation, research data, configuration, helpers, earlier worktrees,
rollback files, and published alpha artifacts. Do not replace global binaries as
part of comparative testing.

After the gate, use normal checked GitHub integration and supported installation.
Screen exact outgoing files and metadata. Verify remote acceptance, local
activation, and practical installed use separately. Existing schema rollback and
finite-lived diagnostic-process rules remain in force. See
[the delivery plan](DELIVERY-PLAN.md), [crawl rules](CRAWL.md), and
[source-access requirements](SOURCE-ACCESS.md).
