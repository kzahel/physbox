# Local LLM Unit Test Plan

## Goal Description
Create a unit test to verify that the backend can successfully interface with the local Llama model and produce deterministic output. This ensures the local LLM setup is working correctly before proceeding with other tasks.

## User Review Required
- [ ] Determinism: I will set `temperature` to `0` for the test to maximize determinism, but LLMs can still vary slightly. I will check for non-empty valid responses rather than exact string matches if exact matching proves too brittle.

## Proposed Changes

### Code Changes
- **`backend/src/services/llm.service.ts`**:
    - Update `getLLM()` to accept an optional `temperature` parameter (defaulting to 0.7).
    - Pass this parameter to the `ChatLlamaCpp` / `ChatOpenAI` constructor.

### Tests
- **`backend/test/llm.test.ts`** (New):
    - Test case: "Local LLM should return a response".
    - Setup: Call `getLLM({ temperature: 0 })`.
    - Action: Invoke with a simple prompt (e.g., "Say 'Hello'").
    - Assertion: Verify response is not empty and contains expected content (e.g., "Hello").

## Verification Plan

### Automated Tests
- Run `npm test backend/test/llm.test.ts` (or `npx jest backend/test/llm.test.ts`).
