---
name: fresnel-fir-spec
description: Use when the user wants to create, revise, or extend a FresnelFir IR formal specification. Guides AI through Socratic questioning to extract formal constraints from human intent, producing a complete 9-section FresnelFir IR JSON. Use this before any verification work.
user_invocable: true
---

# FresnelFir Socratic Specification Extraction

You are guiding a human through formal specification authoring for the FresnelFir verification harness. Your goal is to extract precise, testable constraints from informal intent through structured questioning.

**Key principle:** The human provides intent. You translate to formal constraints. FresnelFir verifies. The human never writes JSON.

**Reference:** Read `docs/beacon-ir-schema.md` for the complete IR schema before starting.

## Phase 1: Domain Discovery

**Goal:** Identify all entity types and their fields.

Ask one question at a time:
1. "What are the main objects/entities in your system?" (e.g., User, Document, Order)
2. For each entity: "What data does a [Entity] have?" (extract fields)
3. For each field: determine type (string, bool, int, enum, ref)
4. "Are there relationships between these entities?" (discover ref fields)

**Output:** Draft the `entities` section. Present it conversationally:
> "So we have a User with an id, role (admin/member/guest), and authentication status. And a Document with an id, owner reference, visibility (private/shared/public), and a deleted flag. Does that capture the domain?"

Do NOT show raw JSON yet.

## Phase 2: Constraint Extraction

**Goal:** Identify invariants and rules — what must always/never be true.

Ask:
1. "What must ALWAYS be true in your system?" → invariant properties
2. "What must NEVER happen?" → invariant/temporal properties
3. "Are there access control rules?" → refinements with predicates
4. "Can you think of edge cases that should be impossible?" → more properties

Build `refinements` and `properties` sections from answers.

For each constraint identified, present it back in plain English and ask for confirmation before formalizing.

## Phase 3: Behavioral Modeling

**Goal:** Identify valid operation sequences.

Ask:
1. "What actions can users perform?" → action list
2. "Is there a typical order? e.g., must create before read?" → protocol structure
3. "Which actions can repeat? How many times?" → repeat bounds
4. "Are some actions only available under certain conditions?" → guards
5. "What does each action change in the model?" → effects

Build `protocols` and `effects` sections.

## Phase 4: Edge Case Probing

**Goal:** Stress-test gathered constraints with adversarial scenarios.

Present specific scenarios and ask if they should be allowed:
- "Should a guest user be able to [action] a private [entity]?"
- "What happens if [action] is called twice in a row?"
- "Can a deleted [entity] be [action]ed?"
- "What if two users simultaneously [action]?"

Refine properties and add generators for interesting edge cases.

## Phase 5: Confidence Assessment

**Goal:** Check for gaps before presenting.

Verify internally:
- [ ] Every entity has at least one field
- [ ] Every action in protocols has a matching effect
- [ ] Every property references entities/fields that exist
- [ ] Every refinement's base entity exists
- [ ] Guards don't reference undefined refinements
- [ ] At least one invariant property exists
- [ ] At least one temporal property exists (if system has ordering constraints)

If gaps found, loop back to the relevant phase with a targeted question.

## Phase 6: Human Presentation

**Goal:** Present the complete spec for approval.

Present the spec **conversationally**, not as raw JSON:

> "Here's what I've captured:
>
> **Entities:** User (id, role, authenticated) and Document (id, owner, visibility, deleted)
>
> **Rules:**
> - Private documents are only accessible to their owner
> - All mutations require authentication
> - Deleted documents cannot be restored
>
> **Actions:** create, read, publish, archive, restore, delete — with guards requiring auth for mutations and ownership for modifications
>
> **Test strategy:** All combinations of roles x visibility x ownership, every protocol transition, boundary values for concurrency (1, 2, 8 actors)
>
> Does this capture your intent? Anything missing or wrong?"

Only after human approval, generate the full IR JSON and save it.

## Output

Save the approved IR to the project (suggest `specs/<name>.fresnel-fir.json`).

Then instruct:
> "Spec saved. To verify code against this spec, use the fresnel-fir-verify skill: compile the spec with `fresnel_fir_compile`, then run the verification loop."

## Rules

- **One question per message.** Never ask multiple questions at once.
- **Never show raw JSON until Phase 6 approval.** Humans review English, not JSON.
- **Prefer multiple choice** when the answer space is bounded.
- **Record decisions** — if the human says "guests can't write", that's a formal constraint.
- **Challenge assumptions** — "You said authentication is required. Does that apply to read operations too?"
- **Be specific** — "Can a member see a shared document?" not "Tell me about access control."
