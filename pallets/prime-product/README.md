# Prime Product Pallet

The purpose of the pallet is to help users to define and solve the following problem:

With the equation

N = a * b

for a given uint N, find a and b, knowing both a and b are prime integers

Example:

Problem: N = 6

Solution: a = 2, b = 3

- Any user can submit new problem and set a non-zero reward for a solution.

- Any user can submit solution candidate for yet unsolved problem, and if correct, receive 80% of the problem's reward.

- The rest 20% of the reward stays in pallet's "treasury pool".

The following data should be stored on-chain:

- unsolved and solved problems,
- correct solutions.

## Assumptions and restrictions

Every time a user submits a problem, the prize/reward that he offers is going to be locked until someone submits a correct solution.

As this was a simple implementation I didn't want to overcomplicate it so these are the current limitations:

- There's no way to revoke a problem.
- Users will be able to submit only one unsolved problem at a time. That means that if a user sends a new problem after already having sent another one which has not been solved, that will cause an error and his problem will be rejected.
