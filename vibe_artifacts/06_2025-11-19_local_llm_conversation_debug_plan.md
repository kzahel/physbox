# Fix Frontend-Backend Connection

## Goal
Ensure the frontend can successfully communicate with the backend. Currently, there is a port mismatch (Frontend uses 3000, Backend defaults to 1234) and a potential issue with `localhost` on Android emulators.

## Proposed Changes

### Backend
#### [NEW] [backend/.env](file:///home/kgraehl/code/backend-task-agy/backend/.env)
- Create `.env` file (if not exists) based on `.env.example`.
- Set `PORT=3000` to match the frontend's expectation.
- (User needs to provide OpenAI Key, but I will add a placeholder or keep it empty for now as that is a separate task).

### Frontend
#### [MODIFY] [ChatScreen.tsx](file:///home/kgraehl/code/backend-task-agy/frontend/src/ChatScreen.tsx)
- Import `Platform` from `react-native`.
- Define `API_URL` dynamically:
  - Android: `http://10.0.2.2:3000/api/chat`
  - iOS/Web: `http://localhost:3000/api/chat`
- Use `API_URL` in the `axios.post` call.

## Verification Plan

### Manual Verification
1.  Start backend: `cd backend && npm run start`
2.  Start frontend (in a separate terminal, user action): `cd frontend && npm run start`
3.  Send a message in the chat interface.
4.  Verify in backend logs that a request was received (even if it fails later due to OpenAI missing).
5.  Verify in frontend that it doesn't immediately show a network error (it might show an error about OpenAI, but that's the next task).
