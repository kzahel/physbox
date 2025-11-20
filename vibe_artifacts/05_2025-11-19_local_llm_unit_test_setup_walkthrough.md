# Local LLM Setup Walkthrough

## Changes Made

### Architecture
- **Abstraction Layer**: Implemented `llm.service.ts` using **LangChain**. This allows switching between `local` (Llama) and `openai` providers via environment variables.
- **Local Model**: Integrated `node-llama-cpp` to run GGUF models locally.
- **Controller Update**: Refactored `chat.controllers.ts` to use the new `getLLM()` factory instead of calling OpenAI directly.

### Configuration
- **Environment Variables**: Added `LLM_PROVIDER` and `LOCAL_MODEL_PATH` to `.env`.
- **Model Download**: Added `scripts/download-model.ts` (and ran `curl`) to download **TinyLlama-1.1B** (approx. 637MB).

## Verification Results

### Automated Tests
- **Build**: Passed (`tsc --noEmit`).
- **Unit Test**: Created `backend/test/llm.test.ts` which runs a deterministic test against the local model using `node --test`.
    - Command: `npm test` (runs `tsx --test test/llm.test.ts`)
    - Result: Passed (Model responded with "Hello World").

### Manual Verification
- **Model Existence**: Verified `backend/models/tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf` exists (637MB).
- **Configuration**: `.env` is set to `LLM_PROVIDER=local`.

## How to Use
1.  **Switch Provider**: Change `LLM_PROVIDER` in `backend/.env` to `openai` or `local`.
2.  **Run Backend**: `npm start` in `backend` directory.
3.  **Chat**: The backend will now use the selected provider.

## Next Steps
- Proceed with the remaining tasks in `README.md`.
