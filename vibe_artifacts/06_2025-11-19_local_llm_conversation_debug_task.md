# Task: Investigate Server Exit Issue

- [x] Check model file existence <!-- id: 0 -->
- [/] Verify server process stability <!-- id: 1 -->
- [/] Check for unhandled exceptions <!-- id: 2 -->
- [ ] Report findings to user <!-- id: 3 -->

# Upcoming Tasks (from README)
- [ ] The backend cannot communicate with the OpenAI API, fix this
- [ ] The frontend allows the user to specify a language but currently this is ignored. Support this feature.
- [ ] Allow users to specify a language level (CEFR) in the frontend and prompt the LLM to respect this.
- [ ] Add another API endpoint that corrects each user message after sending. Display the correction to the user in the frontend.
- [ ] Add input / argument validation to the controllers
- [ ] Implement the error handling middleware and use it to properly inform the frontend about issues.
- [ ] Refactor code that belongs into a service or middleware
- [ ] Make the data in the mock db persist over multiple starts of the backend
- [ ] Add support for other LLM models and make this configurable over a parameter.
- [ ] Generalize the `getAIResponse` function
- [ ] Handle OpenAI communication failures gracefully
- [ ] Add a retry mechanism to the communication with the openAI API
- [ ] Increase context by including previous 2 messages
- [ ] Ask users in a first message about what they want to talk about
- [ ] Give feedback after 5 messages
- [ ] Make the conversation more interesting
- [ ] Write tests for the API endpoints
- [ ] Fix visible type errors
- [ ] Fix linter errors
- [ ] Fix linter warnings
