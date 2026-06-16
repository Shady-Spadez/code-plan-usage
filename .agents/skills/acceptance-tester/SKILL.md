---
name: acceptance-tester
description: Verify code changes pass acceptance criteria, fix issues automatically, and converge requirements into a features document. Use after implementing a feature or making behavioral changes to the coding-plan-widget project.
---

# Acceptance Tester

This skill performs acceptance testing on recent code changes, verifies they meet all acceptance criteria, fixes any issues found, and converges the living requirements document into a stable features document.

## When to Use This Skill

Activate this skill whenever:
- A feature implementation is complete and ready for verification
- The user asks to "verify", "validate", "test", or "accept" changes
- The user wants to finalize a feature and update documentation
- After a series of code changes that affect behavior

## Instructions

### 1. Read All Relevant Documents

At the start, read both documents:

```
.agents/skills/requirements-tracker/requirements.md
.agents/skills/requirements-tracker/features.md
```

- `requirements.md` — the living document with all features and their acceptance criteria
- `features.md` — the stable document listing completed, verified features

If `features.md` does not exist yet, note that it will be created as part of the convergence step.

### 2. Identify Features to Verify

From `requirements.md`, identify features with status `🔧 In Progress` or recently changed features. These are the candidates for acceptance testing.

### 3. Launch Sub-Agent for Verification

For each feature to verify, spawn a sub-agent with the following instructions:

```
You are an acceptance tester for the coding-plan-widget project. Your job is to verify that a feature meets its acceptance criteria.

## Feature to Verify
[Copy the full feature section from requirements.md, including all acceptance criteria]

## Instructions
1. Read the relevant source files to understand the implementation
2. For each acceptance criterion, determine if it is met by the current code
3. Check for regressions: does this feature break any other feature listed in requirements.md?
4. Check code quality: are there obvious bugs, error handling gaps, or edge cases not covered?

## Output Format
Return your findings in this exact format:

### Acceptance Criteria Results
- [x] or [ ] Criterion description — brief explanation
- ...

### Regression Check
- [List any features that may be broken, or "None found"]

### Issues Found
- [List each issue with file path and line numbers if applicable]

### Overall Verdict
- PASS: All criteria met, no regressions, no issues
- FAIL: [Brief summary of what failed]
```

Collect the sub-agent's findings.

### 4. Fix Issues (If Any)

If the sub-agent reports FAIL:

1. Read the relevant source files to understand the issues
2. Fix each issue one at a time
3. After fixing, re-run the sub-agent verification for that feature
4. Repeat until the sub-agent returns PASS

If an issue cannot be fixed (e.g., requires external dependencies, is a known limitation), document it clearly and ask the user whether to proceed.

### 5. Update Requirements Document

After all features pass acceptance:

1. Update the status of each verified feature in `requirements.md` from `🔧 In Progress` to `✅ Implemented`
2. Check off any remaining unchecked acceptance criteria

### 6. Converge to Features Document

After updating `requirements.md`, converge the completed features into `features.md`:

1. If `features.md` does not exist, create it at `.agents/skills/requirements-tracker/features.md`
2. For each feature with status `✅ Implemented` in `requirements.md`, add a condensed entry to `features.md` using this format:

```markdown
## [Feature Name]

**Verified**: YYYY-MM-DD

### Summary
[One or two sentences describing what the feature does]

### Key Files
- `src/file.rs` — [brief role]
```

3. The `features.md` document should be a clean, stable reference — no status tracking, no unchecked criteria, no implementation notes. It represents the current state of the application.

### 7. Report Summary

After completing all steps, provide a summary:

```
## Acceptance Testing Complete

### Verified Features
- Feature A: PASS
- Feature B: PASS

### Issues Fixed
- [Issue 1] in src/file.rs
- [Issue 2] in src/other.rs

### Documents Updated
- requirements.md: statuses updated
- features.md: converged with N features
```

## Important Notes

- Always read the full requirements document before starting verification — this ensures you catch regressions
- The sub-agent should review the actual source code, not just the requirements document
- If a feature has no code changes but is listed as `🔧 In Progress`, ask the user whether it should be verified
- Do not remove information from `requirements.md` — it is the living document. Only update statuses and checkboxes
- `features.md` is the stable snapshot — it should only contain verified, implemented features
